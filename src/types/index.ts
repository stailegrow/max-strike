export interface Server {
  id: string
  name: string
  address: string
  port: number
  protocol: 'vless' | 'trojan' | 'hysteria2'
  uuid: string
  flow?: string
  sni?: string
  publicKey?: string
  shortId?: string
  security?: string
  fingerprint?: string
  type?: string
  ping?: number
  status: 'active' | 'standby' | 'error'
}

export interface Subscription {
  id: string
  name: string
  url: string
  servers: Server[]
  createdAt: number
  lastUpdate?: number
}

export interface Stats {
  upload: number
  download: number
  totalUpload: number
  totalDownload: number
}
