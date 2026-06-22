import { useState, useRef } from 'react'

interface QrScannerProps {
  onScan: (data: string) => void
}

export function QrScanner({ onScan }: QrScannerProps) {
  const [error, setError] = useState<string | null>(null)
  const fileInputRef = useRef<HTMLInputElement>(null)

  const handleFileSelect = async (e: React.ChangeEvent<HTMLInputElement>) => {
    const file = e.target.files?.[0]
    if (!file) return

    const reader = new FileReader()
    reader.onload = async (event) => {
      const dataUrl = event.target?.result as string
      
      // Динамически импортируем jsQR
      const { default: jsQR } = await import('jsqr')
      
      const img = new Image()
      img.onload = () => {
        const canvas = document.createElement('canvas')
        const ctx = canvas.getContext('2d')
        if (!ctx) return

        canvas.width = img.width
        canvas.height = img.height
        ctx.drawImage(img, 0, 0)
        
        const imageData = ctx.getImageData(0, 0, canvas.width, canvas.height)
        const code = jsQR(imageData.data, imageData.width, imageData.height)

        if (code) {
          onScan(code.data)
        } else {
          setError('QR-код не найден на изображении')
        }
      }
      img.src = dataUrl
    }
    reader.readAsDataURL(file)
  }

  return (
    <div className="qr-scanner">
      <label className="qr-upload-btn">
        <input 
          ref={fileInputRef}
          type="file" 
          accept="image/*" 
          onChange={handleFileSelect}
          style={{ display: 'none' }}
        />
        📁 Выбрать изображение
      </label>
      
      {error && (
        <p className="qr-error">❌ {error}</p>
      )}
    </div>
  )
}
