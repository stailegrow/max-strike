import { useState, useEffect } from 'react'
import { Store } from '@tauri-apps/plugin-store'
import { invoke } from '@tauri-apps/api/core'
import type { Subscription, Server } from '../types'

const STORE_FILE = 'app_data.json'
const SUBSCRIPTIONS_KEY = 'subscriptions'

export function useSubscriptions() {
  const [subscriptions, setSubscriptions] = useState<Subscription[]>([])
  const [store, setStore] = useState<Store | null>(null)
  const [loading, setLoading] = useState(false)

  useEffect(() => {
    async function initStore() {
      const newStore = await Store.load(STORE_FILE)
      setStore(newStore)
      
      const saved = await newStore.get<Subscription[]>(SUBSCRIPTIONS_KEY)
      if (saved && Array.isArray(saved)) {
        const validSubs = saved.filter(s => s && s.id && s.servers)
        setSubscriptions(validSubs)
      }
    }
    
    initStore()
  }, [])

  useEffect(() => {
    if (store) {
      store.set(SUBSCRIPTIONS_KEY, subscriptions)
      store.save()
    }
  }, [subscriptions, store])

  const addSubscription = async (name: string, url: string) => {
    setLoading(true)
    try {
      // Используем Rust команду вместо браузерного fetch
      const servers = await invoke<Server[]>('fetch_subscription', { url })
      
      const newSub: Subscription = {
        id: Date.now().toString(),
        name,
        url,
        servers,
        createdAt: Date.now(),
        lastUpdate: Date.now(),
      }
      
      setSubscriptions([...subscriptions, newSub])
      return servers.length
    } finally {
      setLoading(false)
    }
  }

  const addSubscriptionWithServers = async (name: string, servers: Server[]) => {
    const newSub: Subscription = {
      id: Date.now().toString(),
      name,
      url: '',
      servers,
      createdAt: Date.now(),
      lastUpdate: Date.now(),
    }
    
    setSubscriptions([...subscriptions, newSub])
  }

  const updateSubscription = async (id: string) => {
    const sub = subscriptions.find(s => s.id === id)
    if (!sub || !sub.url) return
    
    setLoading(true)
    try {
      const servers = await invoke<Server[]>('fetch_subscription', { url: sub.url })
      
      setSubscriptions(
        subscriptions.map(s => 
          s.id === id 
            ? { ...s, servers, lastUpdate: Date.now() }
            : s
        )
      )
    } finally {
      setLoading(false)
    }
  }

  const removeSubscription = (id: string) => {
    setSubscriptions(subscriptions.filter(sub => sub.id !== id))
  }

  const allServers: Server[] = subscriptions
    .filter(sub => sub && Array.isArray(sub.servers))
    .flatMap(sub => sub.servers)
    .filter(s => s && s.id)

  return {
    subscriptions,
    allServers,
    loading,
    addSubscription,
    addSubscriptionWithServers,
    updateSubscription,
    removeSubscription,
  }
}
