import { createRouter, createWebHashHistory, type RouteRecordRaw } from "vue-router";

const routes: RouteRecordRaw[] = [
  {
    path: "/",
    redirect: "/projects",
  },
  {
    path: "/projects",
    name: "projects",
    component: () => import("@/views/ProjectsView.vue"),
    meta: { title: "项目" },
  },
  {
    path: "/proxies",
    name: "proxies",
    component: () => import("@/views/ProxiesView.vue"),
    meta: { title: "Proxy 列表" },
  },
  {
    path: "/servers",
    name: "servers",
    component: () => import("@/views/ServersView.vue"),
    meta: { title: "frps 服务端" },
  },
  {
    path: "/dashboard/:serverId",
    name: "dashboard",
    component: () => import("@/views/DashboardView.vue"),
    meta: { title: "服务端 Dashboard" },
  },
  {
    path: "/ports",
    name: "ports",
    component: () => import("@/views/PortsView.vue"),
    meta: { title: "本机端口" },
  },
  {
    path: "/settings",
    name: "settings",
    component: () => import("@/views/SettingsView.vue"),
    meta: { title: "设置" },
  },
];

export const router = createRouter({
  history: createWebHashHistory(),
  routes,
});
