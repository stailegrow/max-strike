import { parseShareLink } from './parseShareLink'
import type { Server } from '../types'

export function parseSubscriptionContent(content: string): Server[] {
  let decoded = content.trim()
  
  try {
    if (/^[A-Za-z0-9+/=\s]+$/.test(decoded) && decoded.length > 50) {
      decoded = atob(decoded.replace(/\s/g, ''))
    }
  } catch (e) {
    console.log('Not base64, using as plain text')
  }
  
  const lines = decoded.split('\n').map(l => l.trim()).filter(l => l.length > 0)
  const servers: Server[] = []
  
  for (const line of lines) {
    if (line.startsWith('#') || line.startsWith('//')) continue
    
    const server = parseShareLink(line)
    if (server) {
      servers.push(server)
    }
  }
  
  return servers
}

export async function fetchAndParseSubscription(url: string): Promise<Server[]> {
  const response = await fetch(url)
  
  if (!response.ok) {
    throw new Error(`HTTP ${response.status}: ${response.statusText}`)
  }
  
  const content = await response.text()
  return parseSubscriptionContent(content)
}
