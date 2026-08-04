import { createRouter, createWebHashHistory } from 'vue-router'
import HomeView from '../views/HomeView.vue'
import ImportView from '../views/ImportView.vue'
import MatchView from '../views/MatchView.vue'
import ExportView from '../views/ExportView.vue'
import DebugView from '../views/DebugView.vue'

// ponytail: 全部同步导入。Tauri 桌面应用体积小，同步导入让 Vite 启动时一次性编译所有模块，
// 避免 dev 模式下懒加载 chunk 首次切换时的按需编译/大 JS 解析导致切页卡几分钟。
const routes = [
  { path: '/', name: 'home', component: HomeView },
  { path: '/import', name: 'import', component: ImportView },
  { path: '/match', name: 'match', component: MatchView },
  { path: '/export', name: 'export', component: ExportView },
  { path: '/debug', name: 'debug', component: DebugView },
]

export const router = createRouter({
  history: createWebHashHistory(),
  routes,
})
