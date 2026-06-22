import type { Server } from '../types'

export function parseShareLink(link: string): Server | null {
  try {
    link = link.trim()
    
    if (link.startsWith('vless://')) {
      return parseVless(link)
    } else if (link.startsWith('trojan://')) {
      return parseTrojan(link)
    } else if (link.startsWith('hysteria2://') || link.startsWith('hy2://')) {
      return parseHysteria2(link)
    }
    
    console.warn('Unknown protocol:', link.substring(0, 20))
    return null
  } catch (error) {
    console.error('Parse error:', error, link)
    return null
  }
}

function parseVless(link: string): Server {
  const withoutProtocol = link.substring(8)
  const [mainPart, name = 'VLESS Server'] = withoutProtocol.split('#')
  const [userInfoAndHost, queryString = ''] = mainPart.split('?')
  
  const atIndex = userInfoAndHost.indexOf('@')
  if (atIndex === -1) throw new Error('Invalid vless format')
  
  const uuid = userInfoAndHost.substring(0, atIndex)
  const hostPort = userInfoAndHost.substring(atIndex + 1)
  
  const colonIndex = hostPort.lastIndexOf(':')
  const address = colonIndex === -1 ? hostPort : hostPort.substring(0, colonIndex)
  const port = colonIndex === -1 ? 443 : parseInt(hostPort.substring(colonIndex + 1))
  
  const params = new URLSearchParams(queryString)
  
  return {
    id: crypto.randomUUID(),
    name: decodeURIComponent(name),
    protocol: 'vless',
    address,
    port,
    uuid,
    flow: params.get('flow') || undefined,
    sni: params.get('sni') || undefined,
    publicKey: params.get('pbk') || undefined,
    shortId: params.get('sid') || undefined,
    security: params.get('security') || undefined,
    fingerprint: params.get('fp') || undefined,
    type: params.get('type') || 'tcp',
    status: 'standby',
  }
}

function parseTrojan(link: string): Server {
  const withoutProtocol = link.substring(9)
  const [mainPart, name = 'Trojan Server'] = withoutProtocol.split('#')
  const [userInfoAndHost, queryString = ''] = mainPart.split('?')
  
  const atIndex = userInfoAndHost.indexOf('@')
  if (atIndex === -1) throw new Error('Invalid trojan format')
  
  const password = userInfoAndHost.substring(0, atIndex)
  const hostPort = userInfoAndHost.substring(atIndex + 1)
  
  const colonIndex = hostPort.lastIndexOf(':')
  const address = colonIndex === -1 ? hostPort : hostPort.substring(0, colonIndex)
  const port = colonIndex === -1 ? 443 : parseInt(hostPort.substring(colonIndex + 1))
  
  const params = new URLSearchParams(queryString)
  
  return {
    id: crypto.randomUUID(),
    name: decodeURIComponent(name),
    protocol: 'trojan',
    address,
    port,
    uuid: password,
    sni: params.get('sni') || undefined,
    type: params.get('type') || 'tcp',
    status: 'standby',
  }
}

function parseHysteria2(link: string): Server {
  const protocol = link.startsWith('hy2://') ? 'hy2://' : 'hysteria2://'
  const withoutProtocol = link.substring(protocol.length)
  
  const [mainPart, name = 'Hysteria2 Server'] = withoutProtocol.split('#')
  const [userInfoAndHost, queryString = ''] = mainPart.split('?')
  
  const atIndex = userInfoAndHost.indexOf('@')
  if (atIndex === -1) throw new Error('Invalid hysteria2 format')
  
  const auth = userInfoAndHost.substring(0, atIndex)
  const hostPort = userInfoAndHost.substring(atIndex + 1)
  
  const colonIndex = hostPort.lastIndexOf(':')
  const address = colonIndex === -1 ? hostPort : hostPort.substring(0, colonIndex)
  const port = colonIndex === -1 ? 443 : parseInt(hostPort.substring(colonIndex + 1))
  
  const params = new URLSearchParams(queryString)
  
  return {
    id: crypto.randomUUID(),
    name: decodeURIComponent(name),
    protocol: 'hysteria2',
    address,
    port,
    uuid: auth,
    sni: params.get('sni') || undefined,
    status: 'standby',
  }
}
