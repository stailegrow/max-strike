# MAX STRIKE

Современный VPN-клиент на базе Tauri + Xray с удобной системой маршрутизации.

## 🚀 Возможности

- Поддержка протоколов: **VLESS**, **Trojan**, **Hysteria2**
- Умная маршрутизация трафика (Россия / Зарубеж)
- Обход LAN
- Блокировка рекламы через AdGuard DNS
- Системный прокси (GNOME)
- Импорт подписок (по ссылке, QR-код, файл)
- Сохранение всех настроек

## 📁 Структура проекта

```bash
max-strike/
├── src/                          # Frontend (React + TypeScript)
│   ├── App.tsx                   # Главный компонент приложения
│   ├── main.tsx                  # Точка входа
│   ├── hooks/
│   │   ├── useSettings.ts        # Настройки (тема, язык, маршрутизация)
│   │   └── useSubscriptions.ts   # Работа с подписками и серверами
│   ├── components/               # Компоненты (импорт QR, файлов и т.д.)
│   ├── i18n.ts                   # Локализация (RU/EN)
│   └── assets/                   # Иконки и статические файлы
│
├── src-tauri/                    # Backend (Rust + Tauri)
│   ├── src/
│   │   └── lib.rs                # Основная логика (парсинг, подключение, routing)
│   ├── tauri.conf.json           # Конфигурация Tauri (окно, иконки, bundling)
│   └── Cargo.toml                # Зависимости Rust
│
├── core/                         # Бинарные файлы ядра
│   ├── max-strike-core           # Go-core (обработка подключений)
│   └── xray                      # Xray-core
│
├── src-tauri/target/release/bundle/deb/   # Готовые .deb пакеты
└── README.md
Описание основных файлов

src-tauri/src/lib.rs - Основная логика приложения (парсинг подписок, подключение, маршрутизация, логи)
src/hooks/useSettings.ts - Управление настройками (тема, язык, маршрутизация)
src/hooks/useSubscriptions.ts - Работа с подписками и списком серверов
src/App.tsx - Главный интерфейс приложения
src-tauri/tauri.conf.json - Конфигурация окна, иконок, ресурсов и сборки
core/max-strike-core - Go-ядро, которое запускает Xray с нужной конфигурацией


Установка
Bashsudo apt install ./MAX_STRIKE_1.0.5_amd64.deb

Требования

Ubuntu 22.04 / Debian 11+
GNOME Desktop Environment (для работы системного прокси)
libwebkit2gtk-4.1-0, libgtk-3-0, libssl3
