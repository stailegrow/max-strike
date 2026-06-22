import { useState, useEffect } from 'react'
import { useSubscriptions } from './hooks/useSubscriptions'
import { useSettings } from './hooks/useSettings'
import { invoke } from '@tauri-apps/api/core'
import { open } from '@tauri-apps/plugin-shell'
import { open as openDialog } from '@tauri-apps/plugin-dialog'
import { readTextFile } from '@tauri-apps/plugin-fs'
import { writeText } from '@tauri-apps/plugin-clipboard-manager'
import type { Server } from './types'
import { translations } from './i18n'
import './App.css'

const APP_VERSION = '1.0.5'

interface LogEntry {
  timestamp: string
  level: string
  message: string
}

interface RoutingConfig {
  blockAds: boolean
  bypassLan: boolean
  splitRouting: boolean
  region: 'russia' | 'all'
}

function App() {
  const { 
    subscriptions, allServers, loading,
    addSubscription, addSubscriptionWithServers,
    updateSubscription, removeSubscription 
  } = useSubscriptions()
  
  const { theme, setTheme, language, setLanguage } = useSettings()
  
  const [currentPage, setCurrentPage] = useState<'home' | 'subscriptions' | 'settings' | 'interface' | 'routing' | 'logs' | 'about'>('home')
  const [connectedServerId, setConnectedServerId] = useState<string | null>(null)
  const [connecting, setConnecting] = useState(false)
  const [selectedServerId, setSelectedServerId] = useState<string | null>(null)
  const [importMode, setImportMode] = useState<'none' | 'url' | 'qr' | 'file'>('none')
  const [newSubName, setNewSubName] = useState('')
  const [newSubUrl, setNewSubUrl] = useState('')
  const [importLoading, setImportLoading] = useState(false)
  const [importError, setImportError] = useState<string | null>(null)
  const [modal, setModal] = useState<'none' | 'terms' | 'privacy'>('none')
  const [logs, setLogs] = useState<LogEntry[]>([])
  const [notification, setNotification] = useState<string | null>(null)
  
  // Routing state
  const [routingConfig, setRoutingConfig] = useState<RoutingConfig>({
    blockAds: false,
    bypassLan: true,
    splitRouting: true,
    region: 'russia'
  })

  const t = translations[language]

  useEffect(() => {
    if (currentPage === 'logs') {
      loadLogs()
      const interval = setInterval(loadLogs, 2000)
      return () => clearInterval(interval)
    }
  }, [currentPage])

  useEffect(() => {
    loadRoutingConfig()
  }, [])

  const loadLogs = async () => {
    try {
      const logData = await invoke<LogEntry[]>('get_logs')
      setLogs(logData)
    } catch (error) {
      console.error('Failed to load logs:', error)
    }
  }

  const loadRoutingConfig = async () => {
    try {
      const config = await invoke<RoutingConfig>('get_routing_config')
      setRoutingConfig(config)
    } catch (error) {
      console.error('Failed to load routing config:', error)
    }
  }

  const saveRoutingConfig = async (config: RoutingConfig) => {
    try {
      await invoke('save_routing_config', { config })
      setRoutingConfig(config)
    } catch (error) {
      console.error('Failed to save routing config:', error)
    }
  }

  const handleCopyLogs = async () => {
    const logText = logs.map(l => `[${l.timestamp}] [${l.level}] ${l.message}`).join('\n')
    await writeText(logText)
    showNotification(t.logsCopied)
  }

  const handleClearLogs = async () => {
    await invoke('clear_logs')
    setLogs([])
    showNotification(t.logsCleared)
  }

  const showNotification = (message: string) => {
    setNotification(message)
    setTimeout(() => setNotification(null), 3000)
  }

  const handleServerClick = (server: Server) => {
    if (connecting) return
    if (connectedServerId === server.id) {
      handleDisconnect()
    } else {
      setSelectedServerId(server.id)
    }
  }

  const handleConnect = async (server: Server) => {
    if (connecting) return
    setConnecting(true)
    try {
      await invoke('connect_to_server', { server })
      setConnectedServerId(server.id)
      setSelectedServerId(null)
      await invoke('set_system_proxy', { enabled: true })
    } catch (error) {
      console.error('Connection failed:', error)
    } finally {
      setConnecting(false)
    }
  }

  const handleDisconnect = async () => {
    setConnecting(true)
    try {
      await invoke('disconnect_from_server')
      await invoke('set_system_proxy', { enabled: false })
      setConnectedServerId(null)
      setSelectedServerId(null)
    } catch (error) {
      console.error('Disconnect failed:', error)
    } finally {
      setConnecting(false)
    }
  }

  const handleOpenLink = async (url: string) => {
    await open(url)
  }

  const handleUrlImport = async () => {
    if (!newSubName.trim() || !newSubUrl.trim()) return
    setImportLoading(true)
    setImportError(null)
    try {
      await addSubscription(newSubName.trim(), newSubUrl.trim())
      setNewSubName('')
      setNewSubUrl('')
      setImportMode('none')
    } catch (error) {
      setImportError(String(error))
    } finally {
      setImportLoading(false)
    }
  }

  const handleQrFileSelect = async (e: React.ChangeEvent<HTMLInputElement>) => {
    const file = e.target.files?.[0]
    if (!file) return
    setImportLoading(true)
    setImportError(null)
    try {
      const reader = new FileReader()
      reader.onload = async (event) => {
        const dataUrl = event.target?.result as string
        const img = new Image()
        img.onload = async () => {
          const canvas = document.createElement('canvas')
          const ctx = canvas.getContext('2d')
          if (!ctx) return
          canvas.width = img.width
          canvas.height = img.height
          ctx.drawImage(img, 0, 0)
          const { default: jsQR } = await import('jsqr')
          const imageData = ctx.getImageData(0, 0, canvas.width, canvas.height)
          const code = jsQR(imageData.data, imageData.width, imageData.height)
          if (!code) {
            setImportError('QR-код не распознан')
            setImportLoading(false)
            return
          }
          const data = code.data
          if (data.startsWith('http://') || data.startsWith('https://')) {
            await addSubscription('QR Subscription', data)
          } else {
            const servers = await invoke<Server[]>('parse_subscription_content_string', { content: data })
            await addSubscriptionWithServers('QR Import', servers)
          }
          setImportMode('none')
          setImportLoading(false)
        }
        img.src = dataUrl
      }
      reader.readAsDataURL(file)
    } catch (error) {
      setImportError(String(error))
      setImportLoading(false)
    }
  }

  const handleFileSelect = async () => {
    setImportLoading(true)
    setImportError(null)
    try {
      const selected = await openDialog({
        multiple: false,
        filters: [{ name: 'Config Files', extensions: ['txt', 'json', 'conf', 'yaml', 'yml', ''] }]
      })
      if (!selected || typeof selected !== 'string') {
        setImportLoading(false)
        return
      }
      const content = await readTextFile(selected)
      const servers = await invoke<Server[]>('parse_subscription_content_string', { content })
      const fileName = selected.split('/').pop() || 'Import'
      await addSubscriptionWithServers(fileName, servers)
      setImportMode('none')
    } catch (error) {
      setImportError(String(error))
    } finally {
      setImportLoading(false)
    }
  }

  const connectedServer = allServers.find(s => s.id === connectedServerId)

  const getLogColor = (level: string) => {
    switch (level) {
      case 'ERROR': return 'var(--error)'
      case 'WARN': return 'var(--warning)'
      case 'INFO': return 'var(--info)'
      case 'DEBUG': return 'var(--text-muted)'
      default: return 'var(--text-secondary)'
    }
  }

  return (
    <div className="app">
      <aside className="sidebar">
        <div className="logo">
          <span className="logo-text">MAX STRIKE</span>
        </div>
        <nav className="nav">
          <button className={`nav-btn ${currentPage === 'home' ? 'active' : ''}`} onClick={() => setCurrentPage('home')}>{t.home}</button>
          <button className={`nav-btn ${currentPage === 'subscriptions' ? 'active' : ''}`} onClick={() => setCurrentPage('subscriptions')}>{t.subscriptions}</button>
          <button className={`nav-btn ${currentPage === 'settings' ? 'active' : ''}`} onClick={() => setCurrentPage('settings')}>{t.settings}</button>
          <button className={`nav-btn ${currentPage === 'logs' ? 'active' : ''}`} onClick={() => setCurrentPage('logs')}>{t.logs}</button>
          <button className={`nav-btn ${currentPage === 'about' ? 'active' : ''}`} onClick={() => setCurrentPage('about')}>{t.about}</button>
        </nav>
      </aside>

      <main className="main">
        <header className="header">
          <div style={{ display: 'flex', alignItems: 'center', gap: '12px' }}>
            {(currentPage === 'interface' || currentPage === 'routing') && (
              <button className="btn-back" onClick={() => setCurrentPage('settings')}>←</button>
            )}
            <h1 className="page-title">
              {currentPage === 'home' && t.home}
              {currentPage === 'subscriptions' && t.subscriptions}
              {currentPage === 'settings' && t.settings}
              {currentPage === 'interface' && t.interface}
              {currentPage === 'routing' && t.routing}
              {currentPage === 'logs' && t.logs}
              {currentPage === 'about' && t.about}
            </h1>
          </div>
          <div className={`status ${connectedServer ? 'connected' : ''}`}>
            <span className="status-dot"></span>
            <span>{connectedServer ? connectedServer.name : t.notConnected}</span>
          </div>
        </header>

        <div className="content">
          {currentPage === 'home' && (
            <div className="card">
              <div className="card-header">
                <h2 className="card-title">{t.servers}</h2>
                <span className="text-muted">{allServers.length}</span>
              </div>
              <div className="server-list">
                {allServers.map((server) => {
                  const isConnected = connectedServerId === server.id
                  const isSelected = selectedServerId === server.id
                  return (
                    <div key={server.id} className={`server ${isConnected ? 'connected' : ''} ${isSelected ? 'selected' : ''}`} onClick={() => handleServerClick(server)}>
                      <div className="server-header">
                        <span className="server-name">{server.name}</span>
                        {isConnected && <span className="badge connected">{t.connected}</span>}
                      </div>
                      <div className="server-info">
                        <span>{server.address}:{server.port}</span>
                        <span>•</span>
                        <span>{server.protocol.toUpperCase()}</span>
                        {server.sni && (<><span>•</span><span>SNI: {server.sni}</span></>)}
                      </div>
                      {isSelected && !isConnected && (
                        <button className="btn btn-primary" onClick={(e) => { e.stopPropagation(); handleConnect(server) }} disabled={connecting}>
                          {connecting ? t.connecting : t.connect}
                        </button>
                      )}
                      {isConnected && (
                        <button className="btn btn-danger" onClick={(e) => { e.stopPropagation(); handleDisconnect() }} disabled={connecting}>
                          {connecting ? t.disconnecting : t.disconnect}
                        </button>
                      )}
                    </div>
                  )
                })}
              </div>
            </div>
          )}

          {currentPage === 'subscriptions' && (
            <>
              <div className="card">
                <div className="card-header">
                  <h2 className="card-title">{t.addSubscription}</h2>
                </div>
                <div className="import-grid">
                  <button className="import-btn" onClick={() => { setImportMode('url'); setImportError(null) }}>
                    <span className="import-icon">🔗</span>
                    <span className="import-label">{t.url}</span>
                    <span className="import-hint">{t.link}</span>
                  </button>
                  <button className="import-btn" onClick={() => { setImportMode('qr'); setImportError(null) }}>
                    <span className="import-icon">📷</span>
                    <span className="import-label">{t.qrCode}</span>
                    <span className="import-hint">{t.image}</span>
                  </button>
                  <button className="import-btn" onClick={() => { setImportMode('file'); setImportError(null) }}>
                    <span className="import-icon">📄</span>
                    <span className="import-label">{t.file}</span>
                    <span className="import-hint">{t.config}</span>
                  </button>
                </div>
              </div>

              {importMode === 'url' && (
                <div className="card">
                  <div className="card-header">
                    <h2 className="card-title">{t.url} {t.link}</h2>
                    <button className="btn-close" onClick={() => setImportMode('none')}>✕</button>
                  </div>
                  <div className="subscription-form">
                    <input type="text" className="input" placeholder="Название" value={newSubName} onChange={(e) => setNewSubName(e.target.value)} disabled={importLoading} />
                    <input type="text" className="input" placeholder="URL подписки (https://...)" value={newSubUrl} onChange={(e) => setNewSubUrl(e.target.value)} disabled={importLoading} />
                    <button className="btn btn-full" onClick={handleUrlImport} disabled={importLoading || !newSubName.trim() || !newSubUrl.trim()}>
                      {importLoading ? '⏳ Загрузка...' : '💾 Сохранить'}
                    </button>
                    {importError && <p className="qr-error">{importError}</p>}
                  </div>
                </div>
              )}

              {importMode === 'qr' && (
                <div className="card">
                  <div className="card-header">
                    <h2 className="card-title">{t.qrCode}</h2>
                    <button className="btn-close" onClick={() => setImportMode('none')}>✕</button>
                  </div>
                  <div className="qr-content">
                    <p className="text-muted" style={{ marginBottom: '12px', textAlign: 'center' }}>Выберите изображение с QR-кодом</p>
                    <label className="btn btn-full" style={{ cursor: 'pointer', textAlign: 'center' }}>
                      {importLoading ? '⏳ Сканирование...' : '📁 Выбрать изображение'}
                      <input type="file" accept="image/*" onChange={handleQrFileSelect} style={{ display: 'none' }} disabled={importLoading} />
                    </label>
                    {importError && <p className="qr-error">{importError}</p>}
                  </div>
                </div>
              )}

              {importMode === 'file' && (
                <div className="card">
                  <div className="card-header">
                    <h2 className="card-title">{t.file}</h2>
                    <button className="btn-close" onClick={() => setImportMode('none')}>✕</button>
                  </div>
                  <div className="qr-content">
                    <p className="text-muted" style={{ marginBottom: '12px', textAlign: 'center' }}>Выберите файл с подпиской (.txt, .json, .conf)</p>
                    <button className="btn btn-full" onClick={handleFileSelect} disabled={importLoading}>
                      {importLoading ? '⏳ Загрузка...' : '📁 Выбрать файл'}
                    </button>
                    {importError && <p className="qr-error">{importError}</p>}
                  </div>
                </div>
              )}

              {subscriptions.length > 0 && (
                <div className="card">
                  <div className="card-header">
                    <h2 className="card-title">{t.mySubscriptions} ({subscriptions.length})</h2>
                  </div>
                  <div className="subscription-list">
                    {subscriptions.map((sub) => (
                      <div key={sub.id} className="server" style={{ marginBottom: '8px' }}>
                        <div className="server-header">
                          <span className="server-name">{sub.name}</span>
                          <span className="text-muted">{sub.servers?.length} {t.serversCount}</span>
                        </div>
                        <div className="server-info">{sub.url}</div>
                        <div style={{ display: 'flex', gap: '8px', marginTop: '8px' }}>
                          <button className="btn btn-secondary btn-small" onClick={() => updateSubscription(sub.id)} disabled={loading}>🔄 Обновить</button>
                          <button className="btn btn-danger btn-small" onClick={() => removeSubscription(sub.id)}>🗑 Удалить</button>
                        </div>
                      </div>
                    ))}
                  </div>
                </div>
              )}
            </>
          )}

          {currentPage === 'settings' && (
            <div className="card">
              <div className="card-header">
                <h2 className="card-title">{t.settings}</h2>
              </div>
              <div className="settings-list">
                <div className="setting-item" onClick={() => setCurrentPage('interface')} style={{ cursor: 'pointer' }}>
                  <span className="setting-label">{t.interface}</span>
                  <span>→</span>
                </div>
                <div className="setting-item" onClick={() => setCurrentPage('routing')} style={{ cursor: 'pointer' }}>
                  <span className="setting-label">{t.routing}</span>
                  <span>→</span>
                </div>
              </div>
            </div>
          )}

          {currentPage === 'interface' && (
            <div className="card">
              <div className="card-header">
                <h2 className="card-title">{t.interface}</h2>
              </div>
              <div className="settings-list">
                <div className="setting-item">
                  <span className="setting-label">{t.language}</span>
                  <select className="select" value={language} onChange={(e) => setLanguage(e.target.value as 'ru' | 'en')}>
                    <option value="ru">Русский</option>
                    <option value="en">English</option>
                  </select>
                </div>
                <div className="setting-item">
                  <span className="setting-label">{t.theme}</span>
                  <select className="select" value={theme} onChange={(e) => setTheme(e.target.value)}>
                    <option value="dark">{t.dark} (Purple)</option>
                    <option value="light">{t.light}</option>
                    <option value="blue">Blue</option>
                    <option value="green">Green</option>
                    <option value="red">Red</option>
                    <option value="orange">Orange</option>
                    <option value="cyan">Cyan</option>
                  </select>
                </div>
              </div>
            </div>
          )}

          {currentPage === 'routing' && (
            <div className="card">
              <div className="card-header">
                <h2 className="card-title">{t.routingRules}</h2>
              </div>
              <div className="settings-list">
                <div className="setting-item toggle-item">
                  <div>
                    <div className="setting-label">{t.blockAds}</div>
                    <div className="setting-desc">{t.blockAdsDesc}</div>
                  </div>
                  <label className="toggle-switch">
                    <input 
                      type="checkbox" 
                      checked={routingConfig.blockAds}
                      onChange={(e) => saveRoutingConfig({ ...routingConfig, blockAds: e.target.checked })}
                    />
                    <span className="toggle-slider"></span>
                  </label>
                </div>
                
                <div className="setting-item toggle-item">
                  <div>
                    <div className="setting-label">{t.bypassLan}</div>
                    <div className="setting-desc">{t.bypassLanDesc}</div>
                  </div>
                  <label className="toggle-switch">
                    <input 
                      type="checkbox" 
                      checked={routingConfig.bypassLan}
                      onChange={(e) => saveRoutingConfig({ ...routingConfig, bypassLan: e.target.checked })}
                    />
                    <span className="toggle-slider"></span>
                  </label>
                </div>
                
                <div className="setting-item toggle-item">
                  <div>
                    <div className="setting-label">{t.splitRouting}</div>
                    <div className="setting-desc">{t.splitRoutingDesc}</div>
                  </div>
                  <label className="toggle-switch">
                    <input 
                      type="checkbox" 
                      checked={routingConfig.splitRouting}
                      onChange={(e) => saveRoutingConfig({ ...routingConfig, splitRouting: e.target.checked })}
                    />
                    <span className="toggle-slider"></span>
                  </label>
                </div>
                
                {routingConfig.splitRouting && (
                  <div className="setting-item">
                    <span className="setting-label">{t.region}</span>
                    <select 
                      className="select" 
                      value={routingConfig.region}
                      onChange={(e) => saveRoutingConfig({ ...routingConfig, region: e.target.value as 'russia' | 'all' })}
                    >
                      <option value="russia">{t.regionRussia}</option>
                      <option value="all">{t.regionAll}</option>
                    </select>
                  </div>
                )}
              </div>
            </div>
          )}

          {currentPage === 'logs' && (
            <div className="card">
              <div className="card-header">
                <h2 className="card-title">{t.logs} ({logs.length})</h2>
                <div style={{ display: 'flex', gap: '8px' }}>
                  <button className="btn btn-secondary btn-small" onClick={handleCopyLogs}>{t.copyLogs}</button>
                  <button className="btn btn-danger btn-small" onClick={handleClearLogs}>{t.clearLogs}</button>
                </div>
              </div>
              <div className="logs-container">
                {logs.length === 0 ? (
                  <p className="text-muted" style={{ textAlign: 'center', padding: '2rem' }}>{t.noLogs}</p>
                ) : (
                  <div className="logs-list">
                    {logs.slice().reverse().map((log, i) => (
                      <div key={i} className="log-entry">
                        <span className="log-timestamp">{log.timestamp}</span>
                        <span className="log-level" style={{ color: getLogColor(log.level) }}>[{log.level}]</span>
                        <span className="log-message">{log.message}</span>
                      </div>
                    ))}
                  </div>
                )}
              </div>
            </div>
          )}

          {currentPage === 'about' && (
            <div className="card">
              <div className="about-page">
                <div className="about-header">
                  <div className="about-logo">
                    <img src="/icon.png" alt="MAX STRIKE" width="80" height="80" />
                  </div>
                  <div className="about-info">
                    <h2 className="about-title">MAX STRIKE</h2>
                    <p className="about-version">v{APP_VERSION} beta</p>
                  </div>
                </div>
                <div className="about-divider"></div>
                <div className="about-links">
                  <div className="about-link-item" onClick={() => handleOpenLink('https://github.com/stailegrow/max-strike')}>
                    <span>{t.sourceCode}</span>
                    <svg width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2">
                      <path d="M18 13v6a2 2 0 0 1-2 2H5a2 2 0 0 1-2-2V8a2 2 0 0 1 2-2h6"></path>
                      <polyline points="15 3 21 3 21 9"></polyline>
                      <line x1="10" y1="14" x2="21" y2="3"></line>
                    </svg>
                  </div>
                  <div className="about-link-item" onClick={() => handleOpenLink('https://t.me/MAX_STRIKE_VPN')}>
                    <span>{t.telegramChannel}</span>
                    <svg width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2">
                      <path d="M18 13v6a2 2 0 0 1-2 2H5a2 2 0 0 1-2-2V8a2 2 0 0 1 2-2h6"></path>
                      <polyline points="15 3 21 3 21 9"></polyline>
                      <line x1="10" y1="14" x2="21" y2="3"></line>
                    </svg>
                  </div>
                  <div className="about-divider-small"></div>
                  <div className="about-link-item" onClick={() => setModal('terms')}>
                    <span>{t.termsOfUse}</span>
                    <svg width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2">
                      <path d="M18 13v6a2 2 0 0 1-2 2H5a2 2 0 0 1-2-2V8a2 2 0 0 1 2-2h6"></path>
                      <polyline points="15 3 21 3 21 9"></polyline>
                      <line x1="10" y1="14" x2="21" y2="3"></line>
                    </svg>
                  </div>
                  <div className="about-link-item" onClick={() => setModal('privacy')}>
                    <span>{t.privacyPolicy}</span>
                    <svg width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2">
                      <path d="M18 13v6a2 2 0 0 1-2 2H5a2 2 0 0 1-2-2V8a2 2 0 0 1 2-2h6"></path>
                      <polyline points="15 3 21 3 21 9"></polyline>
                      <line x1="10" y1="14" x2="21" y2="3"></line>
                    </svg>
                  </div>
                </div>
              </div>
            </div>
          )}
        </div>
      </main>

      {notification && (
        <div className="notification">
          {notification}
        </div>
      )}

      {modal !== 'none' && (
        <div className="modal-overlay" onClick={() => setModal('none')}>
          <div className="modal" onClick={(e) => e.stopPropagation()}>
            <div className="modal-header">
              <h2 className="modal-title">{modal === 'terms' ? t.termsTitle : t.privacyTitle}</h2>
              <button className="btn-close" onClick={() => setModal('none')}>✕</button>
            </div>
            <div className="modal-content">
              <pre className="modal-text">{modal === 'terms' ? t.termsText : t.privacyText}</pre>
            </div>
            <div className="modal-footer">
              <button className="btn btn-primary" style={{ marginTop: 0, width: 'auto' }} onClick={() => setModal('none')}>{t.close}</button>
            </div>
          </div>
        </div>
      )}
    </div>
  )
}

export default App
