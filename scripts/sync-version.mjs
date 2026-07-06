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
const newCargo = cargo.replace(/^version\s*=\s*"[^"]*"/m, `version = "${version}"`)
if (newCargo !== cargo) {
  writeFileSync(cargoPath, newCargo)
  console.log(`Cargo.toml version -> ${version}`)
} else {
  console.log(`Cargo.toml version already ${version}, skip`)
}

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
const newTauri = JSON.stringify(tauriObj, null, 2) + '\n'
if (newTauri !== tauri) {
  writeFileSync(tauriPath, newTauri)
  console.log(`tauri.conf.json version -> ${version}`)
} else {
  console.log(`tauri.conf.json version already ${version}, skip`)
}

console.log(`\n版本已同步至 ${version}`)
