import { useState, useRef, useEffect } from 'react'
import jsQR from 'jsqr'

interface QrScannerProps {
  onScan: (data: string) => void
  onClose: () => void
}

export function QrScanner({ onScan, onClose }: QrScannerProps) {
  const [filePreview, setFilePreview] = useState<string | null>(null)
  const [error, setError] = useState<string | null>(null)
  const [scanning, setScanning] = useState(false)
  const canvasRef = useRef<HTMLCanvasElement>(null)
  const fileInputRef = useRef<HTMLInputElement>(null)

  const decodeQR = async (dataUrl: string) => {
    setScanning(true)
    setError(null)
    
    try {
      const img = new Image()
      img.crossOrigin = "anonymous"
      
      await new Promise((resolve, reject) => {
        img.onload = resolve
        img.onerror = reject
        img.src = dataUrl
      })

      const canvas = document.createElement('canvas')
      const ctx = canvas.getContext('2d')
      
      if (!ctx) {
        throw new Error('Не удалось получить контекст canvas')
      }

      canvas.width = img.width
      canvas.height = img.height
      ctx.drawImage(img, 0, 0)
      
      const imageData = ctx.getImageData(0, 0, canvas.width, canvas.height)
      const code = jsQR(imageData.data, imageData.width, imageData.height)

      if (code) {
        console.log('QR decoded:', code.data)
        onScan(code.data)
      } else {
        setError('❌ QR-код не распознан. Попробуйте другое изображение.')
      }
    } catch (err) {
      console.error('Decode error:', err)
      setError('❌ Ошибка при чтении QR-кода')
    } finally {
      setScanning(false)
    }
  }

  const handleFileSelect = (e: React.ChangeEvent<HTMLInputElement>) => {
    const file = e.target.files?.[0]
    if (!file) return

    const reader = new FileReader()
    reader.onload = (event) => {
      const dataUrl = event.target?.result as string
      setFilePreview(dataUrl)
      decodeQR(dataUrl)
    }
    reader.readAsDataURL(file)
  }

  return (
    <div className="qr-scanner-inner">
      <p className="qr-hint">
        Выберите изображение с QR-кодом подписки
      </p>
      
      <label className="qr-upload-btn">
        <input 
          ref={fileInputRef}
          type="file" 
          accept="image/*" 
          onChange={handleFileSelect}
          style={{ display: 'none' }}
        />
        {scanning ? '⏳ Сканирование...' : '📁 Выбрать изображение'}
      </label>
      
      {filePreview && (
        <div className="qr-preview">
          <img src={filePreview} alt="QR preview" />
        </div>
      )}
      
      {error && (
        <div className="qr-error">
          {error}
        </div>
      )}
      
      <div className="qr-info">
        <p><strong>Поддерживается:</strong></p>
        <ul>
          <li>🔗 URL подписки (https://...)</li>
          <li>🔵 vless:// ссылки</li>
          <li>🟢 trojan:// ссылки</li>
          <li>🟡 hysteria2:// ссылки</li>
        </ul>
      </div>
    </div>
  )
}
