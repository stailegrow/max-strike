import { useState, useEffect } from 'react'
import { Store } from '@tauri-apps/plugin-store'
import type { Language } from '../i18n'

const STORE_FILE = 'app_data.json'
const THEME_KEY = 'theme'
const LANGUAGE_KEY = 'language'

export function useSettings() {
  const [theme, setThemeState] = useState<string>('light')
  const [language, setLanguageState] = useState<Language>('ru')
  const [store, setStore] = useState<Store | null>(null)

  // Инициализация
  useEffect(() => {
    async function init() {
      const newStore = await Store.load(STORE_FILE)
      setStore(newStore)
      
      const savedTheme = await newStore.get<string>(THEME_KEY)
      const savedLang = await newStore.get<Language>(LANGUAGE_KEY)
      
      if (savedTheme) setThemeState(savedTheme)
      if (savedLang) setLanguageState(savedLang)
    }
    init()
  }, [])

  // Применение темы
  useEffect(() => {
    document.documentElement.setAttribute('data-theme', theme)
  }, [theme])

  // Сохранение при изменении
  useEffect(() => {
    if (store) {
      store.set(THEME_KEY, theme)
      store.save()
    }
  }, [theme, store])

  useEffect(() => {
    if (store) {
      store.set(LANGUAGE_KEY, language)
      store.save()
    }
  }, [language, store])

  const setTheme = (newTheme: string) => setThemeState(newTheme)
  const setLanguage = (newLang: Language) => setLanguageState(newLang)

  return { theme, setTheme, language, setLanguage }
}
