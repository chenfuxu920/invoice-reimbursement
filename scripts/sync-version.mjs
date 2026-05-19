import { readFileSync, writeFileSync } from 'fs'
import { resolve, dirname } from 'path'
import { fileURLToPath } from 'url'

const __dirname = dirname(fileURLToPath(import.meta.url))
const root = resolve(__dirname, '..')

const pkg = JSON.parse(readFileSync(resolve(root, 'package.json'), 'utf8'))
const version = pkg.version

if (!version) {
  console.error('package.json 中未找到 version')
  process.exit(1)
}

const cargoPath = resolve(root, 'src-tauri', 'Cargo.toml')
let cargo = readFileSync(cargoPath, 'utf8')
cargo = cargo.replace(/^version\s*=\s*"[^"]*"/m, `version = "${version}"`)
writeFileSync(cargoPath, cargo)
console.log(`Cargo.toml version -> ${version}`)

const tauriPath = resolve(root, 'src-tauri', 'tauri.conf.json')
let tauri = readFileSync(tauriPath, 'utf8')
const tauriObj = JSON.parse(tauri)
tauriObj.version = version
tauriObj.productName = tauriObj.productName.replace(/v[\d.]+/, `v${version}`)
if (tauriObj.app?.windows) {
  for (const w of tauriObj.app.windows) {
    if (w.title) {
      w.title = w.title.replace(/v[\d.]+/, `v${version}`)
    }
  }
}
writeFileSync(tauriPath, JSON.stringify(tauriObj, null, 2) + '\n')
console.log(`tauri.conf.json version -> ${version}`)

console.log(`\n版本已同步至 ${version}`)
