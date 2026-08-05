export interface CityStandard {
  name: string
  standard: number
}

export interface ProvinceStandard {
  name: string
  defaultStandard: number
  cities: CityStandard[]
}

export interface StandardSet {
  id: string
  name: string
  defaultHotelStandard: number
  provinces: ProvinceStandard[]
}

export interface ReimbursementConfig {
  cityTransportDaily: number
  mealSubsidyDaily: number
  activeStandardSetId: string
  standardSets: StandardSet[]
}
