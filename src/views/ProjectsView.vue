<script setup lang="ts">
import { computed, ref } from "vue";
import {
  Card,
  CardContent,
  CardDescription,
  CardHeader,
  CardTitle,
} from "@/components/ui/card";
import { Button } from "@/components/ui/button";
import { Badge } from "@/components/ui/badge";
import { Plus, FolderKanban, Pencil, Trash2, Network } from "@lucide/vue";
import ProjectFormDialog from "@/components/forms/ProjectFormDialog.vue";
import ProxyFormDialog from "@/components/forms/ProxyFormDialog.vue";
import { ConfirmDialog } from "@/components/ui/confirm";
import { useDataStore } from "@/stores/data";
import { toast } from "@/components/ui/toast";
import type { Project } from "@/types/store";

const data = useDataStore();

const projectFormOpen = ref(false);
const editingProject = ref<Project | null>(null);

const proxyFormOpen = ref(false);
const proxyDefaultProjectId = ref<string | undefined>(undefined);

const confirmOpen = ref(false);
const pendingDelete = ref<Project | null>(null);
const deleting = ref(false);

function openCreate() {
  editingProject.value = null;
  projectFormOpen.value = true;
}
function openEdit(p: Project) {
  editingProject.value = p;
  projectFormOpen.value = true;
}
function askDelete(p: Project) {
  pendingDelete.value = p;
  confirmOpen.value = true;
}
async function doDelete() {
  if (!pendingDelete.value) return;
  deleting.value = true;
  try {
    await data.deleteProject(pendingDelete.value.id);
    toast.success("已删除");
    confirmOpen.value = false;
  } catch (e) {
    toast.fromError(e);
  } finally {
    deleting.value = false;
  }
}

function addProxyTo(p: Project) {
  proxyDefaultProjectId.value = p.id;
  proxyFormOpen.value = true;
}

const proxyCountByProject = computed(() => {
  const m = new Map<string, number>();
  for (const px of data.proxies) {
    m.set(px.project_id, (m.get(px.project_id) ?? 0) + 1);
  }
  return m;
});
</script>

<template>
  <div class="space-y-6">
    <div class="flex items-center justify-between">
      <div>
        <h2 class="text-2xl font-semibold tracking-tight">项目</h2>
        <p class="text-sm text-muted-foreground">
          所有 Proxy 必须挂在某个项目下，便于命名规范和管理
        </p>
      </div>
      <Button @click="openCreate">
        <Plus class="h-4 w-4" />
        新建项目
      </Button>
    </div>

    <Card v-if="data.projects.length === 0">
      <CardHeader class="flex flex-col items-center text-center">
        <div class="mx-auto mb-2 flex h-12 w-12 items-center justify-center rounded-full bg-secondary">
          <FolderKanban class="h-6 w-6 text-muted-foreground" />
        </div>
        <CardTitle>还没有任何项目</CardTitle>
        <CardDescription>
          先创建一个项目，例如 nas 或 company-vpn，然后在项目下添加 Proxy
        </CardDescription>
      </CardHeader>
      <CardContent class="flex justify-center pb-6">
        <Button variant="outline" @click="openCreate">
          <Plus class="h-4 w-4" />
          创建第一个项目
        </Button>
      </CardContent>
    </Card>

    <div v-else class="grid grid-cols-1 gap-4 lg:grid-cols-2">
      <Card v-for="p in data.projects" :key="p.id">
        <CardHeader>
          <div class="flex items-start justify-between gap-2">
            <div class="min-w-0">
              <CardTitle class="flex items-center gap-2">
                <FolderKanban class="h-4 w-4 text-primary shrink-0" />
                <span class="truncate font-mono">{{ p.name }}</span>
              </CardTitle>
              <CardDescription class="mt-1 line-clamp-2">
                {{ p.description }}
              </CardDescription>
            </div>
            <div class="flex gap-1">
              <Button size="icon" variant="ghost" title="编辑" @click="openEdit(p)">
                <Pencil class="h-4 w-4" />
              </Button>
              <Button
                size="icon"
                variant="ghost"
                class="text-destructive hover:bg-destructive/10 hover:text-destructive"
                title="删除"
                @click="askDelete(p)"
              >
                <Trash2 class="h-4 w-4" />
              </Button>
            </div>
          </div>
        </CardHeader>
        <CardContent>
          <div class="flex items-center justify-between">
            <Badge variant="secondary">
              <Network class="h-3 w-3" />
              {{ proxyCountByProject.get(p.id) ?? 0 }} proxy
            </Badge>
            <Button size="sm" variant="outline" @click="addProxyTo(p)">
              <Plus class="h-4 w-4" />
              添加 Proxy
            </Button>
          </div>
        </CardContent>
      </Card>
    </div>

    <ProjectFormDialog v-model:open="projectFormOpen" :project="editingProject" />
    <ProxyFormDialog
      v-model:open="proxyFormOpen"
      :default-project-id="proxyDefaultProjectId"
    />
    <ConfirmDialog
      v-model:open="confirmOpen"
      :title="`删除项目 “${pendingDelete?.name ?? ''}” ?`"
      description="项目下若还有 proxy 会被拒绝。需要先迁移或删除 proxy。"
      confirm-text="删除"
      destructive
      :loading="deleting"
      @confirm="doDelete"
    />
  </div>
</template>
