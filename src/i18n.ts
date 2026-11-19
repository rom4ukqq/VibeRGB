export type Language = 'en' | 'ru';

const en = {
  launch: 'Startup', loginStart: 'Start when I sign in to Windows', trayStart: 'Opens in the system tray.',
  animatedBackground: 'Animated background', updates: 'Updates', autoCheck: 'Automatically check for updates',
  idle: 'Check for a newer version', current: 'You have the latest version', available: 'Update available',
  checking: 'Checking…', installing: 'Installing update…', updateFailed: 'Could not check for updates',
  checkUpdate: 'Check now', installUpdate: 'Install and restart',
  startupProfile: 'Startup profile', noStartup: 'Do not apply', replace: 'Replace with current scene',
  replaceQuestion: 'Replace the lighting saved in this profile?', animationFailed: 'Animation stopped', refreshIssues: 'Some zones are unavailable.',
  studio: 'Lighting Studio', profiles: 'Profiles', settings: 'Settings', sections: 'Sections',
  language: 'Language', interface: 'Interface', devices: 'Devices', refresh: 'Refresh devices',
  all: 'All devices', active: 'Connected', zones: 'Zones', unavailable: 'Control unavailable',
  installPawnio: 'Install PawnIO', pawnioInstalled: 'PawnIO is installed.',
  restartAsAdmin: 'Restart as administrator',
  pawnioRestart: 'Restart Windows to finish installing PawnIO.',
  stop: 'Stop animation', dismiss: 'Dismiss', preview: 'Preview', noEffect: 'No effect',
  controls: 'Lighting controls', type: 'Effect type', sync: 'Sync', hardware: 'Hardware',
  independent: 'Built-in effects run independently on each controller.',
  zone: 'Zone', allZones: 'All zones', effect: 'Effect', color: 'Color',
  off: 'Off', pulse: 'Pulse', hue: 'Spectrum', breathing: 'Breathing', spectrum: 'Color cycle',
  rainbow: 'Rainbow', picker: 'Color picker', colorFormat: 'Use #RRGGBB.',
  candle: 'Candle', tone: 'Hue', saturation: 'Saturation',
  brightness: 'Brightness', speed: 'Speed', partialBrightness: 'Brightness only changes on supported zones.',
  scanning: 'Looking for controllers…', noZones: 'No available zones.', unsupported: 'This effect is unavailable on the selected zones.',
  apply: 'Apply', updated: 'Zones updated', noUpdates: 'No zones updated.',
  unavailableMode: 'Mode unavailable.', failed: 'Operation failed.', loadFailed: 'Could not load VibeRGB data.',
  eventsFailed: 'Animation error notifications are unavailable.',
  localeFailed: 'Could not change the tray language.',
  profileName: 'New profile name', save: 'Save', saveHint: 'Apply lighting to save a profile.',
  savedProfiles: 'Saved profiles', emptyProfiles: 'No saved profiles.', profileZones: 'Zones in profile',
  newName: 'New name', cancel: 'Cancel', deleteQuestion: 'Delete profile?', remove: 'Delete', rename: 'Rename',
} as const;

const ru: { [K in keyof typeof en]: string } = {
  launch: 'Запуск', loginStart: 'Запускать при входе в Windows', trayStart: 'Приложение откроется в трее.',
  animatedBackground: 'Анимированный фон', updates: 'Обновления', autoCheck: 'Автоматически проверять обновления',
  idle: 'Проверка новой версии', current: 'Установлена последняя версия', available: 'Доступна версия',
  checking: 'Проверяем…', installing: 'Установка обновления…', updateFailed: 'Не удалось проверить обновления',
  checkUpdate: 'Проверить сейчас', installUpdate: 'Установить и перезапустить',
  startupProfile: 'Профиль при запуске', noStartup: 'Не применять', replace: 'Заменить текущей сценой',
  replaceQuestion: 'Заменить подсветку, сохранённую в этом профиле?', animationFailed: 'Анимация остановлена', refreshIssues: 'Часть зон недоступна.',
  studio: 'Студия света', profiles: 'Профили', settings: 'Настройки', sections: 'Разделы',
  language: 'Язык', interface: 'Интерфейс', devices: 'Устройства', refresh: 'Обновить устройства',
  all: 'Все устройства', active: 'Активных', zones: 'Зоны', unavailable: 'Управление недоступно',
  installPawnio: 'Установить PawnIO', pawnioInstalled: 'PawnIO установлен.',
  restartAsAdmin: 'Перезапустить от имени администратора',
  pawnioRestart: 'Перезагрузите Windows, чтобы завершить установку PawnIO.',
  stop: 'Остановить анимацию', dismiss: 'Закрыть уведомление', preview: 'Предпросмотр', noEffect: 'Нет эффекта',
  controls: 'Настройки подсветки', type: 'Тип эффекта', sync: 'Синхронизация', hardware: 'Контроллер',
  independent: 'Встроенные эффекты идут независимо на каждом устройстве.',
  zone: 'Зона', allZones: 'Все зоны', effect: 'Эффект', color: 'Цвет',
  off: 'Выкл.', pulse: 'Пульс', hue: 'Спектр', breathing: 'Дыхание', spectrum: 'Смена цветов',
  rainbow: 'Радуга', picker: 'Палитра', colorFormat: 'Формат — #RRGGBB.',
  candle: 'Свеча', tone: 'Оттенок', saturation: 'Насыщенность',
  brightness: 'Яркость', speed: 'Скорость', partialBrightness: 'Яркость изменится только в поддерживаемых зонах.',
  scanning: 'Поиск контроллеров…', noZones: 'Нет доступных зон.', unsupported: 'Эффект недоступен в выбранных зонах.',
  apply: 'Применить', updated: 'Зон обновлено', noUpdates: 'Зоны не обновлены.',
  unavailableMode: 'Режим недоступен.', failed: 'Операция не выполнена.', loadFailed: 'Не удалось загрузить данные VibeRGB.',
  eventsFailed: 'Ошибки анимации недоступны.',
  localeFailed: 'Не удалось изменить язык меню в трее.',
  profileName: 'Название нового профиля', save: 'Сохранить', saveHint: 'Примените подсветку, чтобы сохранить её в профиль.',
  savedProfiles: 'Сохранённые профили', emptyProfiles: 'Сохранённых профилей пока нет.', profileZones: 'Зон в профиле',
  newName: 'Новое название', cancel: 'Отмена', deleteQuestion: 'Удалить профиль?', remove: 'Удалить', rename: 'Переименовать',
};

export const messages = { en, ru };
export type Messages = typeof ru;

export function sceneName(id: string, fallback: string, labels: Messages) {
  switch (id) {
    case 'solid': return labels.color;
    case 'sync-rainbow': return labels.rainbow;
    case 'off': case 'pulse': case 'candle': case 'hue': case 'breathing': case 'spectrum': case 'rainbow':
      return labels[id];
    default: return fallback;
  }
}

// Controller diagnostics use English on the IPC boundary.
const diagnosticRu = [
 ["No free SMBus address is available for the ENE module.", "Нет свободного адреса SMBus для модуля ENE."],
 ["Could not track the elevated process.", "Не удалось получить доступ к процессу с повышенными правами."],
 [
  "SMBIOS did not report the exact part number. The RGB controller is unconfirmed.",
  "SMBIOS не сообщает точный артикул. RGB-контроллер не подтверждён."
 ],
 [
  "PawnIO is available. No supported ENE DDR4 RGB controller was found.",
  "PawnIO доступен, но поддерживаемый RGB-контроллер ENE DDR4 не найден."
 ],
 [
  "The installed version of PawnIO is missing the required interface.",
  "Установленная версия PawnIO не поддерживает нужный интерфейс."
 ],
 [
  "Two matching GPUs were found. PCI slot selection is unavailable.",
  "Обнаружены две одинаковые видеокарты. Выбор PCI-слота не поддерживается."
 ],
 [
  "Put the signed SmbusPIIX4.bin module next to VibeRGB.exe.",
  "Положите подписанный модуль SmbusPIIX4.bin рядом с VibeRGB.exe."
 ],
 [
  "The Polychrome controller reported no available zones.",
  "Контроллер Polychrome не сообщил доступные зоны."
 ],
 [
  "RGB memory control on this board requires PawnIO.",
  "Для RGB-памяти на этой плате нужен установленный PawnIO."
 ],
[
  "Run VibeRGB as administrator to control RGB memory.",
  "Запустите VibeRGB от имени администратора, чтобы управлять подсветкой памяти."
],
[
  "PawnIO was not installed.",
  "PawnIO не установлен."
],
[
  "PawnIO installation is already running.",
  "Установка PawnIO уже запущена."
],
[
  "Could not track the PawnIO installer.",
  "Не удалось отследить завершение установки PawnIO."
],
 [
  "Windows could not confirm the motherboard model.",
  "Windows не подтвердила модель материнской платы."
 ],
 [
  "No supported MSI Mystic Light GPU found.",
  "Поддерживаемая видеокарта MSI Mystic Light не найдена."
 ],
 [
  "This build does not support the RGB controller.",
  "RGB-контроллер не поддерживается этой сборкой."
 ],
 [
  "NVAPI is unavailable. Check the NVIDIA driver.",
  "NVAPI недоступен. Проверьте драйвер NVIDIA."
 ],
 [
  "Profile names must be 1–64 characters long.",
  "Длина имени профиля должна быть от 1 до 64 символов."
 ],
 [
  "The Program Files directory is unavailable.",
  "Папка Program Files недоступна."
 ],
 [
  "Apply lighting to at least one zone first.",
  "Сначала примените подсветку хотя бы к одной зоне."
 ],
 [
  "SMBus: could not open the system mutex.",
  "SMBus: не удалось открыть системный мьютекс."
 ],
 [
  "Mode unavailable on this memory module.",
  "Режим недоступен для ОЗУ."
 ],
 [
  "The profiles directory is unavailable.",
  "Недоступна папка профилей."
 ],
 [
  "Animation is unavailable on this zone.",
  "У зоны нет подтверждённой поддержки анимаций."
 ],
 [
  "The lighting service did not respond.",
  "Служба подсветки не ответила."
 ],
 [
  "The lighting service is unavailable.",
  "Служба подсветки недоступна."
 ],
 [
  "The profile limit has been reached.",
  "Достигнут предел количества профилей."
 ],
 [
  "Unexpected Polychrome HID response.",
  "Неизвестный формат HID-ответа Polychrome."
 ],
 [
  "NVAPI returned an invalid GPU list.",
  "Некорректный список GPU от NVAPI."
 ],
 [
  "Mode unavailable on the MSI GPU.",
  "Режим недоступен для видеокарты MSI."
 ],
 [
  "Mode unavailable on Polychrome.",
  "Режим недоступен для Polychrome."
 ],
 [
  "Incomplete Polychrome response.",
  "Неполный ответ Polychrome."
 ],
 [
  "NVAPI entry point unavailable.",
  "Точка входа NVAPI недоступна."
 ],
 [
  "Could not load PawnIOLib.dll.",
  "Не удалось загрузить PawnIOLib.dll."
 ],
 [
  "Incomplete Polychrome write.",
  "Неполная запись Polychrome."
 ],
 [
  "Controller worker failed.",
  "Не удалось отправить кадр контроллеру."
 ],
 [
  "Invalid Polychrome zone.",
  "Неверная зона Polychrome."
 ],
 [
  "HID 26CE:01A2 not found",
  "HID 26CE:01A2 не найден"
 ],
 [
  "Polychrome response: ",
  "Ответ Polychrome: "
 ],
 [
  "Device unavailable.",
  "Устройство недоступно."
 ],
 [
  "Effect unavailable.",
  "Эффект недоступен."
 ],
 [
  "Profile not found.",
  "Профиль не найден."
 ],
 [
  "Polychrome write: ",
  "Запись Polychrome: "
 ],
 [
  "The SMBus is busy.",
  "Шина SMBus занята."
 ],
 [
  "Animation stopped",
  "Анимация остановлена"
 ],
 [
  "Zone unavailable.",
  "Зона недоступна."
 ],
 [
  "Mode unavailable.",
  "Режим недоступен."
 ],
 [
  "Profiles file: ",
  "Файл профилей: "
 ],
 [
  "SmbusPIIX4 load",
  "Загрузка SmbusPIIX4"
 ],
 [
  "Module lighting",
  "Подсветка модуля"
 ],
 [
  "initialization",
  "инициализация"
 ],
 [
  "I²C register ",
  "I²C, регистр "
 ],
 [
  "ENE memory · ",
  "ОЗУ ENE · "
 ],
 [
  "GPU lighting",
  "Подсветка видеокарты"
 ],
 [
  "Motherboard",
  "Плата"
 ],
 [
  "Color cycle",
  "Смена цветов"
 ],
 [
  "PawnIO open",
  "Открытие PawnIO"
 ],
 [
  "I/O cover",
  "Панель I/O"
 ],
 [
  "Breathing",
  "Дыхание"
 ],
 [
  "Memory · ",
  "ОЗУ · "
 ],
 [
  "GPU list",
  "список GPU"
 ],
 [
  "Rainbow",
  "Радуга"
 ]
] as const;

export function localizeDiagnostic(value: string, language: Language) {
  if (language === 'en') return value;
  const translated = diagnosticRu.reduce<string>((text, [en, ru]) => text.replaceAll(en, ru), value);
  return translated
    .replace(/Found (\d+) of (\d+) ENE RGB controllers\. ENE modules are listed separately\./g, 'Найдено RGB-контроллеров ENE: $1 из $2. Модули ENE показаны отдельно.')
    .replace(/NVAPI function (0x[\da-f]+) is unavailable\./g, 'Функция NVAPI $1 недоступна.')
    .replace(/\bcode (-?\d+)\./g, 'код $1.');
}
