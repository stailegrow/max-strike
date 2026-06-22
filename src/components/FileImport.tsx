import { useState } from 'react'
import { open } from '@tauri-apps/plugin-dialog'
import { readTextFile } from '@tauri-apps/plugin-fs'
import { invoke } from '@tauri-apps/api/core'
import type { Server } from '../types'

interface FileImportProps {
  onImport: (servers: Server[], content: string) => void
  onClose: () => void
}

export function FileImport({ onImport, onClose }: FileImportProps) {
  const [loading, setLoading] = useState(false)
  const [error, setError] = useState<string | null>(null)
  const [manualContent, setManualContent] = useState('')

  const handleFileSelect = async () => {
    setLoading(true)
    setError(null)
    
    try {
      const selected = await open({
        multiple: false,
        filters: [{
          name: 'Config Files',
          extensions: ['txt', 'json', 'conf', 'yaml', 'yml', '']
        }]
      })
      
      if (!selected || typeof selected !== 'string') {
        setLoading(false)
        return
      }
      
      const content = await readTextFile(selected)
      const servers = await invoke<Server[]>('parse_subscription_content_string', { content })
      
      // Используем имя файла как название подписки
      const fileName = selected.split('/').pop() || 'Import'
      onImport(servers, fileName)
    } catch (e) {
      setError(`Ошибка: ${e}`)
    } finally {
      setLoading(false)
    }
  }

  const handleManualParse = async () => {
    if (!manualContent.trim()) return
    
    setLoading(true)
    setError(null)
    
    try {
      const servers = await invoke<Server[]>('parse_subscription_content_string', { 
        content: manualContent 
      })
      onImport(servers, 'Manual Import')
    } catch (e) {
      setError(`Ошибка: ${e}`)
    } finally {
      setLoading(false)
    }
  }

  return (
    <div className="file-import">
      <div className="file-import-header">
        <h3>📄 Импорт из файла</h3>
        <button className="btn-close" onClick={onClose}>✕</button>
      </div>
      
      <div className="file-import-content">
        <button 
          className="btn btn-full"
          onClick={handleFileSelect}
          disabled={loading}
        >
          {loading ? '⏳ Загрузка...' : '📁 Выбрать файл'}
        </button>
        
        <div className="divider">
          <span>или вставьте содержимое</span>
        </div>
        
        <textarea
          className="input textarea"
          placeholder="Вставьте содержимое подписки или share links (vless://, trojan://, hysteria2://)"
          value={manualContent}
          onChange={(e) => setManualContent(e.target.value)}
          rows={8}
        />
        
        <button
          className="btn btn-full"
          onClick={handleManualParse}
          disabled={loading || !manualContent.trim()}
        >
          {loading ? '⏳ Обработка...' : '🔍 Распознать'}
        </button>
        
        {error && <p className="qr-error">❌ {error}</p>}
      </div>
    </div>
  )
}
