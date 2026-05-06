<template>
  <div class="h-screen flex flex-col bg-gray-50">
    <nav class="flex items-center gap-4 px-6 py-3 bg-white border-b">
      <h1 class="text-lg font-bold">发票报销助手</h1>
      <router-link to="/" class="text-gray-600 hover:text-blue-600">首页</router-link>
      <router-link to="/import" class="text-gray-600 hover:text-blue-600">导入</router-link>
      <router-link to="/match" class="text-gray-600 hover:text-blue-600">匹配</router-link>
      <router-link to="/export" class="text-gray-600 hover:text-blue-600">导出</router-link>
    </nav>
    <main class="flex-1 overflow-auto p-6">
      <router-view />
    </main>
  </div>
</template>
