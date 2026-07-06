import { defineStore } from 'pinia'
import { ref } from 'vue'
import type { PaymentRecord } from '../types'
import { invoke } from '@tauri-apps/api/core'

export const usePaymentStore = defineStore('payment', () => {
  const payments = ref<PaymentRecord[]>([])

  async function importWechatBill(filePath: string) {
    const records: PaymentRecord[] = await invoke('import_wechat_bill', { filePath })
    payments.value.push(...records)
  }

  async function importAlipayBill(filePath: string) {
    const records: PaymentRecord[] = await invoke('import_alipay_bill', { filePath })
    payments.value.push(...records)
  }

  function removePayment(id: string) {
    payments.value = payments.value.filter(p => p.id !== id)
  }

  function clearPayments() {
    payments.value = []
  }

  return { payments, importWechatBill, importAlipayBill, removePayment, clearPayments }
})
