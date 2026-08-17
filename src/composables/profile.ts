import { ref, watch } from 'vue'

const STORAGE_KEY = 'reimbursement-profile'

export interface ReimbursementProfile {
  name: string
  department: string
  companions: number
}

const DEFAULT_PROFILE: ReimbursementProfile = { name: '', department: '', companions: 0 }

function loadProfile(): ReimbursementProfile {
  try {
    const raw = localStorage.getItem(STORAGE_KEY)
    if (raw) return { ...DEFAULT_PROFILE, ...JSON.parse(raw) }
  } catch {
    // 存储内容损坏或不可用，回退默认值
  }
  return { ...DEFAULT_PROFILE }
}

export const profile = ref<ReimbursementProfile>(loadProfile())

// 字段少，直接深监听自动保存，无需防抖
watch(profile, (val) => {
  try {
    localStorage.setItem(STORAGE_KEY, JSON.stringify(val))
  } catch {
    // 存储满或被禁用时静默失败，不影响本次会话
  }
}, { deep: true })

export function useProfile() {
  return { profile }
}
