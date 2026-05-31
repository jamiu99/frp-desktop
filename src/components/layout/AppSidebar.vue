<script setup lang="ts">
import { computed } from "vue";
import { useRoute, RouterLink } from "vue-router";
import { Network, Server, Settings, Plug } from "@lucide/vue";
import { cn } from "@/lib/utils";

const route = useRoute();

const navItems = [
  { to: "/servers", label: "frps 服务端", icon: Server },
  { to: "/proxies", label: "Proxy", icon: Network },
  { to: "/ports", label: "本机端口", icon: Plug },
  { to: "/settings", label: "设置", icon: Settings },
];

const currentPath = computed(() => route.path);
</script>

<template>
  <aside
    class="flex h-full w-56 shrink-0 flex-col border-r bg-card/40 backdrop-blur"
  >
    <div class="flex h-14 items-center gap-2 border-b px-4">
      <img src="/logo.png" alt="logo" class="h-6 w-6" />
      <span class="text-sm font-semibold tracking-tight">frp_desktop</span>
    </div>

    <nav class="flex-1 space-y-1 overflow-y-auto p-2">
      <RouterLink
        v-for="item in navItems"
        :key="item.to"
        :to="item.to"
        :class="
          cn(
            'flex items-center gap-2 rounded-md px-3 py-2 text-sm font-medium transition-colors',
            currentPath.startsWith(item.to)
              ? 'bg-secondary text-foreground'
              : 'text-muted-foreground hover:bg-secondary/60 hover:text-foreground',
          )
        "
      >
        <component :is="item.icon" class="h-4 w-4" />
        <span>{{ item.label }}</span>
      </RouterLink>
    </nav>

    <div class="border-t p-3 text-xs text-muted-foreground">
      <p>v0.1.4</p>
    </div>
  </aside>
</template>
