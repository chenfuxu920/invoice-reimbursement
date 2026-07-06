<template>
  <div class="h-screen flex flex-col bg-gray-50">
    <nav class="flex items-center gap-4 px-6 py-3 bg-white border-b shadow-sm">
      <h1 class="text-lg font-bold text-blue-600">发票报销助手 v{{ version }}</h1>
      <router-link to="/" class="nav-link">首页</router-link>
      <router-link to="/import" class="nav-link">导入</router-link>
      <router-link to="/match" class="nav-link">匹配</router-link>
      <router-link to="/export" class="nav-link">导出</router-link>
      <router-link to="/templates" class="nav-link">模板</router-link>
    </nav>
    <main class="flex-1 overflow-auto p-6">
      <router-view />
    </main>
  </div>
</template>

<script setup lang="ts">
import pkg from "../package.json";
const version = pkg.version;
</script>

<style scoped>
@reference "tailwindcss";
.nav-link {
  @apply px-3 py-1 rounded text-gray-600 hover:text-blue-600 hover:bg-blue-50 transition-colors;
}
.nav-link.router-link-active {
  @apply text-blue-600 bg-blue-50 font-medium;
}
</style>
