use std::{
    fs::{self, File},
    io::{self, Read},
    path::{Path, PathBuf},
    time::Duration,
};

use interprocess::local_socket::{
    tokio::{prelude::*, Listener, Stream},
    ListenerOptions, Name,
};
use serde::{de::DeserializeOwned, Deserialize, Serialize};
use tokio::io::{AsyncBufRead, AsyncBufReadExt, AsyncReadExt, AsyncWriteExt, BufReader};
use tokio::time::timeout;
use uuid::Uuid;

use super::{Reply, Request};

const ENDPOINT_FILE: &str = "automation.json";
const PROTOCOL_VERSION: u32 = 1;
const HANDSHAKE_TIMEOUT: Duration = Duration::from_secs(3);
const REPLY_TIMEOUT: Duration = Duration::from_secs(20);
pub const MAX_REQUEST_BYTES: usize = 64 * 1024;
const MAX_REPLY_BYTES: usize = 16 * 1024 * 1024;

#[derive(Deserialize, Serialize)]
struct Endpoint {
    version: u32,
    address: String,
    proof: Uuid,
}

pub struct LocalListener {
    listener: Listener,
    endpoint: Endpoint,
    path: PathBuf,
}

impl LocalListener {
    pub async fn bind(data_dir: &Path) -> io::Result<Self> {
        let address = socket_address(data_dir)?;
        let options = listener_options(socket_name(&address)?)?;
        let listener = open_listener(options, &address).await?;
        let endpoint = Endpoint {
            version: PROTOCOL_VERSION,
            address,
            proof: Uuid::new_v4(),
        };

        let path = data_dir.join(ENDPOINT_FILE);
        let mut file = tempfile::NamedTempFile::new_in(data_dir)?;
        serde_json::to_writer(file.as_file_mut(), &endpoint)?;
        file.persist(&path)?;
        Ok(Self {
            listener,
            endpoint,
            path,
        })
    }

    pub async fn accept(&self) -> io::Result<Connection> {
        let stream = self.listener.accept().await?;
        verify_peer(&stream)?;
        Ok(Connection {
            io: BufReader::new(stream),
            proof: self.endpoint.proof,
        })
    }
}

impl Drop for LocalListener {
    fn drop(&mut self) {
        if read_endpoint(&self.path).is_ok_and(|endpoint| endpoint.proof == self.endpoint.proof) {
            let _ = fs::remove_file(&self.path);
        }
    }
}

pub struct Connection {
    io: BufReader<Stream>,
    proof: Uuid,
}

impl Connection {
    pub async fn request(&mut self) -> io::Result<Request> {
        timeout(HANDSHAKE_TIMEOUT, async {
            write_frame(self.io.get_mut(), &self.proof, 128).await?;
            read_frame(&mut self.io, MAX_REQUEST_BYTES).await
        })
        .await?
    }

    pub async fn reply(&mut self, reply: &Reply) -> io::Result<()> {
        timeout(
            HANDSHAKE_TIMEOUT,
            write_frame(self.io.get_mut(), reply, MAX_REPLY_BYTES),
        )
        .await?
    }
}

pub async fn call(request: &Request) -> Result<Reply, String> {
    #[derive(Deserialize)]
    struct AppConfig {
        identifier: String,
    }
    let config: AppConfig = serde_json::from_str(include_str!("../../tauri.conf.json"))
        .map_err(|_| "无法读取 FRP Desktop 应用标识")?;
    let data_dir = dirs::data_dir()
        .ok_or("无法定位 FRP Desktop 数据目录")?
        .join(config.identifier);
    call_in(&data_dir, request).await
}

async fn call_in(data_dir: &Path, request: &Request) -> Result<Reply, String> {
    let endpoint = read_endpoint(&data_dir.join(ENDPOINT_FILE))
        .map_err(|_| "找不到 FRP Desktop 的本机接口，请打开支持 CLI 的桌面版本")?;
    if endpoint.version != PROTOCOL_VERSION {
        return Err("CLI 和 FRP Desktop 的接口版本不同，请使用同一版本".into());
    }
    let name = socket_name(&endpoint.address).map_err(|_| "本机接口地址无效")?;
    let stream = timeout(HANDSHAKE_TIMEOUT, Stream::connect(name))
        .await
        .ok()
        .and_then(Result::ok)
        .ok_or("无法连接 FRP Desktop，请检查应用是否正在运行")?;
    verify_peer(&stream).map_err(|_| "无法验证 FRP Desktop 的本机连接")?;
    let mut io = BufReader::new(stream);
    let proof: Uuid = timeout(HANDSHAKE_TIMEOUT, read_frame(&mut io, 128))
        .await
        .ok()
        .and_then(Result::ok)
        .ok_or("FRP Desktop 未完成本机验证")?;
    if proof != endpoint.proof {
        return Err("FRP Desktop 本机连接已失效，请重新查询".into());
    }
    timeout(
        HANDSHAKE_TIMEOUT,
        write_frame(io.get_mut(), request, MAX_REQUEST_BYTES),
    )
    .await
    .ok()
    .and_then(Result::ok)
    .ok_or("无法发送请求，请重新查询当前状态")?;
    timeout(REPLY_TIMEOUT, read_frame(&mut io, MAX_REPLY_BYTES))
        .await
        .ok()
        .and_then(Result::ok)
        .ok_or("FRP Desktop 未返回结果，请先查询当前状态再决定是否重试".into())
}

fn read_endpoint(path: &Path) -> io::Result<Endpoint> {
    let file = File::open(path)?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::MetadataExt;
        let meta = file.metadata()?;
        if !meta.is_file() || meta.uid() != effective_uid() || meta.mode() & 0o077 != 0 {
            return Err(io::Error::new(
                io::ErrorKind::PermissionDenied,
                "unsafe endpoint file",
            ));
        }
    }
    let mut bytes = Vec::new();
    file.take(4096).read_to_end(&mut bytes)?;
    serde_json::from_slice(&bytes).map_err(Into::into)
}

async fn read_frame<T: DeserializeOwned>(
    reader: &mut (impl AsyncBufRead + Unpin),
    limit: usize,
) -> io::Result<T> {
    let mut buffer = Vec::new();
    reader
        .take((limit + 1) as u64)
        .read_until(b'\n', &mut buffer)
        .await?;
    if buffer.len() > limit || !buffer.ends_with(b"\n") {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "incomplete or oversized message",
        ));
    }
    serde_json::from_slice(&buffer).map_err(Into::into)
}

async fn write_frame(
    writer: &mut (impl AsyncWriteExt + Unpin),
    value: &impl Serialize,
    limit: usize,
) -> io::Result<()> {
    let mut frame = serde_json::to_vec(value)?;
    frame.push(b'\n');
    if frame.len() > limit {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "oversized message",
        ));
    }
    writer.write_all(&frame).await?;
    writer.flush().await
}

#[cfg(unix)]
fn effective_uid() -> u32 {
    // SAFETY: geteuid has no arguments or preconditions.
    unsafe { libc::geteuid() }
}

#[cfg(unix)]
fn socket_address(data_dir: &Path) -> io::Result<String> {
    use std::hash::{DefaultHasher, Hash, Hasher};
    use std::os::unix::fs::{DirBuilderExt, MetadataExt};

    let uid = effective_uid();
    // A short private directory also works with macOS's Unix socket path limit.
    let directory = format!("/tmp/frp-desktop-{uid}");
    match fs::DirBuilder::new().mode(0o700).create(&directory) {
        Ok(()) => (),
        Err(error) if error.kind() == io::ErrorKind::AlreadyExists => (),
        Err(error) => return Err(error),
    }
    let meta = fs::symlink_metadata(&directory)?;
    if !meta.is_dir() || meta.uid() != uid || meta.mode() & 0o077 != 0 {
        return Err(io::Error::new(
            io::ErrorKind::PermissionDenied,
            "unsafe local socket directory",
        ));
    }
    let mut hasher = DefaultHasher::new();
    data_dir.hash(&mut hasher);
    Ok(format!("{directory}/{:016x}.sock", hasher.finish()))
}

#[cfg(unix)]
fn socket_name(address: &str) -> io::Result<Name<'_>> {
    use interprocess::local_socket::{GenericFilePath, ToFsName};
    address.to_fs_name::<GenericFilePath>()
}

#[cfg(unix)]
fn listener_options(name: Name<'_>) -> io::Result<ListenerOptions<'_>> {
    Ok(ListenerOptions::new().name(name))
}

#[cfg(unix)]
async fn open_listener(options: ListenerOptions<'_>, address: &str) -> io::Result<Listener> {
    use interprocess::TryClone;
    match options.try_clone()?.create_tokio() {
        Err(error) if error.kind() == io::ErrorKind::AddrInUse => {
            match timeout(HANDSHAKE_TIMEOUT, Stream::connect(socket_name(address)?)).await {
                Ok(Err(inactive)) if inactive.kind() == io::ErrorKind::ConnectionRefused => options
                    .try_overwrite(true)
                    .max_spin_time(HANDSHAKE_TIMEOUT)
                    .create_tokio(),
                _ => Err(error),
            }
        }
        result => result,
    }
}

#[cfg(unix)]
fn verify_peer(stream: &Stream) -> io::Result<()> {
    if stream.peer_creds()?.euid() != Some(effective_uid()) {
        return Err(io::Error::new(
            io::ErrorKind::PermissionDenied,
            "different local user",
        ));
    }
    Ok(())
}

#[cfg(windows)]
fn socket_address(_: &Path) -> io::Result<String> {
    Ok(format!("frp-desktop-{}", Uuid::new_v4()))
}

#[cfg(windows)]
fn socket_name(address: &str) -> io::Result<Name<'_>> {
    use interprocess::local_socket::{GenericNamespaced, ToNsName};
    // Only accept local pipe names generated by the desktop process.
    if !address
        .strip_prefix("frp-desktop-")
        .is_some_and(|id| Uuid::parse_str(id).is_ok())
    {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "invalid named pipe",
        ));
    }
    address.to_ns_name::<GenericNamespaced>()
}

#[cfg(windows)]
fn listener_options(name: Name<'_>) -> io::Result<ListenerOptions<'_>> {
    use interprocess::os::windows::{
        local_socket::ListenerOptionsExt, security_descriptor::SecurityDescriptor,
    };
    // Only the pipe owner may connect. Interprocess also rejects remote pipe clients.
    let descriptor = SecurityDescriptor::deserialize(widestring::u16cstr!("D:P(A;;GA;;;OW)"))?;
    Ok(ListenerOptions::new()
        .name(name)
        .security_descriptor(descriptor))
}

#[cfg(windows)]
async fn open_listener(options: ListenerOptions<'_>, _: &str) -> io::Result<Listener> {
    options.create_tokio()
}

#[cfg(windows)]
fn verify_peer(_: &Stream) -> io::Result<()> {
    // The listener's ACL authenticates the client. The per-launch proof authenticates the server.
    Ok(())
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::*;

    #[tokio::test]
    async fn local_cli_round_trip() {
        let data = tempfile::tempdir().unwrap();
        let listener = LocalListener::bind(data.path()).await.unwrap();
        let server = async {
            let mut session = listener.accept().await.unwrap();
            assert!(matches!(
                session.request().await.unwrap(),
                Request::Logs { lines: 7, .. }
            ));
            session
                .reply(&Reply::Ok {
                    data: json!(["recent local log"]),
                })
                .await
                .unwrap();
        };
        let request = Request::Logs {
            server: "fixture".into(),
            lines: 7,
        };
        let (_, response) = tokio::join!(server, call_in(data.path(), &request));
        assert!(
            matches!(response.unwrap(), Reply::Ok { data } if data == json!(["recent local log"]))
        );
        drop(listener);
        assert!(!data.path().join(ENDPOINT_FILE).exists());
    }

    #[tokio::test]
    async fn cli_sends_nothing_to_an_unverified_server() {
        let data = tempfile::tempdir().unwrap();
        let listener = LocalListener::bind(data.path()).await.unwrap();
        let endpoint_file = data.path().join(ENDPOINT_FILE);
        let mut endpoint = read_endpoint(&endpoint_file).unwrap();
        endpoint.proof = Uuid::new_v4();
        fs::write(&endpoint_file, serde_json::to_vec(&endpoint).unwrap()).unwrap();
        let server = async {
            let mut session = listener.accept().await.unwrap();
            assert!(session.request().await.is_err());
        };
        let (_, response) = tokio::join!(server, call_in(data.path(), &Request::ServerList));
        assert!(response.is_err());
    }
}
