export type PaymentSource = 'Wechat' | 'Alipay'

export interface PaymentRecord {
  id: string
  transaction_id: string
  transaction_time: string
  amount: number
  original_amount: number
  refund_amount: number
  discount: number
  merchant_name: string
  source: PaymentSource
  category: string
  payment_method: string
}
