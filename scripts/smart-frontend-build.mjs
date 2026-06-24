/**
 * 智能前端构建：只在源码或配置变更时才执行 vite build。
 *
 * 原理：比较 src/、index.html、vite.config.ts、package.json 的最新 mtime
 * 与 dist/ 的最新 mtime。源码更新则构建，否则跳过（保持 dist mtime 不变）。
 *
 * 这避免了 tauri build 每次都因 vite 重写 dist 文件而触发 Rust 全量重编译。
 *
 * 用法：node scripts/smart-frontend-build.mjs [--force]
 */
import { readFileSync, statSync, readdirSync, existsSync } from 'fs'
import { resolve, join, dirname } from 'path'
import { fileURLToPath } from 'url'
import { execSync } from 'child_process'

const __dirname = dirname(fileURLToPath(import.meta.url))
const root = resolve(__dirname, '..')

// --force 标志：强制构建
const force = process.argv.includes('--force')

// 需要监视的前端源码目录和文件
const watchPaths = [
  'src',
  'index.html',
  'vite.config.ts',
  'package.json',
  'tsconfig.json',
  'public',
]

const distDir = resolve(root, 'dist')

/**
 * 递归获取目录下最新的文件 mtime（毫秒）
 */
function getNewestMtime(dirPath) {
  if (!existsSync(dirPath)) return 0
  const stat = statSync(dirPath)
  if (stat.isFile()) return stat.mtimeMs

  let newest = stat.mtimeMs
  try {
    for (const entry of readdirSync(dirPath, { withFileTypes: true })) {
      if (entry.name === 'node_modules' || entry.name === '.git') continue
      const fullPath = join(dirPath, entry.name)
      const m = entry.isDirectory() ? getNewestMtime(fullPath) : statSync(fullPath).mtimeMs
      if (m > newest) newest = m
    }
  } catch { /* 忽略权限错误 */ }
  return newest
}

// 计算源码最新 mtime
let srcNewest = 0
for (const p of watchPaths) {
  const full = resolve(root, p)
  if (existsSync(full)) {
    const m = getNewestMtime(full)
    if (m > srcNewest) srcNewest = m
  }
}

// 计算 dist 最新 mtime
const distNewest = getNewestMtime(distDir)

if (force || srcNewest > distNewest) {
  if (force) {
    console.log('Frontend build: forced')
  } else {
    console.log(`Frontend build: source newer than dist (${new Date(srcNewest).toISOString()} > ${new Date(distNewest).toISOString()})`)
  }
  execSync('npx vite build', { stdio: 'inherit', cwd: root })
} else {
  console.log(`Frontend build: skipped (dist up-to-date, newest src ${new Date(srcNewest).toISOString()})`)
}
