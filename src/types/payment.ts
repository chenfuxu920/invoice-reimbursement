export type PaymentSource = 'Wechat' | 'Alipay'

export interface PaymentRecord {
  id: string
  transaction_id: string
  transaction_time: string
  amount: number
  merchant_name: string
  source: PaymentSource
  category: string
}
