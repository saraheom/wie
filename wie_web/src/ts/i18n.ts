import { AppLanguage } from './app_settings';

type Dict = Record<string, string>;

const en: Dict = {
  'brand.games': 'My Games', 'header.logs': '🐞 Logs', 'header.import': '＋ Import',
  'sort.recent': 'Recently Played', 'sort.name': 'Name', 'sort.favorites': 'Favorites',
  'library.empty.title': 'No games yet', 'library.empty.body': 'Import a WIPI or J2ME ZIP/JAR once. It will stay in your library for future launches.', 'library.empty.import': 'Import Game',
  'settings.title': 'WIPI Player Settings', 'settings.subtitle': 'Global library and import defaults',
  'settings.defaults': 'New Game Defaults', 'settings.orientation': 'Orientation', 'settings.screenSize': 'Screen size',
  'orientation.portrait': 'Portrait', 'orientation.landscape': 'Landscape',
  'display.native': 'Original 240 × 320', 'display.compact': 'Compact', 'display.fit': 'Fit', 'display.large': 'Large', 'display.max': 'Maximum',
  'settings.defaults.note': 'These defaults apply only to games imported after you change them. Existing games keep their own settings.',
  'settings.mobile': 'iPhone & Mobile', 'settings.keepAwake': 'Keep screen awake while playing', 'settings.keepAwake.note': 'Prevents the display from sleeping during a game when the platform allows it.',
  'settings.language': 'Language', 'settings.appLanguage': 'App language', 'lang.en': 'English', 'lang.ko': '한국어',
  'settings.language.note': 'Changes the WIPI Player interface language. Game content is not translated.', 'common.done': 'Done', 'common.cancel': 'Cancel', 'common.confirm': 'Confirm',
  'player.display': 'Display', 'player.controls': 'Controls', 'player.save': 'Save Management', 'player.audio': 'Audio',
  'player.customize': '🎮 Customize Controls', 'player.manageSaves': '💾 Manage Saves', 'player.effects': '🔊 Effects',
  'player.display.note': "Screen size changes presentation only. The game's internal WIPI framebuffer remains unchanged.",
  'player.controls.note': 'Move and resize the D-pad and number pad. Portrait and landscape layouts are saved separately for this game.',
  'player.save.note': "Back up, restore, export, import, or erase this game's normal in-game save data.",
  'diagnostics.title': 'Diagnostics (testing only)', 'diagnostics.armExp': 'Arm/Reset EXP Trace', 'diagnostics.view': 'View Log', 'diagnostics.export': 'Export Log', 'diagnostics.clear': 'Clear',
  'controls.edit': 'Edit Controls', 'controls.hint': 'Drag the D-pad or number pad directly on the game screen.', 'controls.dpad': 'D-pad', 'controls.numpad': 'Number pad',
  'controls.size': 'Size', 'controls.spacing': 'Spacing', 'controls.opacity': 'Opacity', 'controls.starting': 'Starting layouts', 'controls.classic': 'Classic', 'controls.spacious': 'Spacious', 'controls.compact': 'Compact', 'controls.reset': 'Reset', 'controls.visible': 'Visible keys',
  'game.edit': 'Edit Game', 'game.displayName': 'Display name', 'game.cover': '🖼 Choose Custom Cover', 'game.saveChanges': 'Save Changes',
  'actions.play': '▶ Play', 'actions.edit': '✎ Edit Game', 'actions.favorite': '★ Favorite / Unfavorite', 'actions.display': '▣ Display Settings', 'actions.controls': '🎮 Customize Controls', 'actions.saves': '💾 Manage Saves', 'actions.cover': '🖼 Change Cover', 'actions.export': '⇧ Export WIPI Entry', 'actions.delete': 'Delete from Library',
  'saves.subtitle': 'Normal in-game save data', 'saves.create': '＋ Create Backup', 'saves.import': 'Import Backup', 'saves.local': 'Local Backups', 'saves.stored': 'Stored inside WIPI Player', 'saves.none': 'No backups yet.', 'saves.current': 'Current Save Data', 'saves.eraseNote': "Erase only the game's current save. Backups listed above are kept.", 'saves.erase': 'Erase Current In-Game Save', 'saves.restore': 'Restore', 'saves.export': 'Export', 'saves.delete': 'Delete',
  'tutorial.library': 'Game Library', 'tutorial.playing': 'Playing again', 'tutorial.saves': 'Game saves', 'tutorial.keyboard': 'Keyboard', 'tutorial.dontShow': "Don't show again",
  'count.game': 'game', 'count.games': 'games', 'badge.portrait': 'Portrait', 'badge.landscape': 'Landscape',
  'sort.aToZ': 'A to Z', 'sort.zToA': 'Z to A', 'sort.oldest': 'Oldest first', 'sort.newest': 'Newest first', 'sort.nonFavorites': 'Non-favorites first', 'sort.favoritesFirst': 'Favorites first',
  'toast.favoriteAdded': 'Added to favorites.', 'toast.favoriteRemoved': 'Removed from favorites.'
};

const ko: Dict = {
  'brand.games': '내 게임', 'header.logs': '🐞 로그', 'header.import': '＋ 불러오기',
  'sort.recent': '최근 실행', 'sort.name': '이름', 'sort.favorites': '즐겨찾기',
  'library.empty.title': '게임이 없습니다', 'library.empty.body': 'WIPI 또는 J2ME ZIP/JAR 파일을 한 번 가져오면 이후에도 라이브러리에 보관됩니다.', 'library.empty.import': '게임 가져오기',
  'settings.title': 'WIPI Player 설정', 'settings.subtitle': '라이브러리 및 가져오기 기본 설정',
  'settings.defaults': '새 게임 기본값', 'settings.orientation': '화면 방향', 'settings.screenSize': '화면 크기',
  'orientation.portrait': '세로', 'orientation.landscape': '가로',
  'display.native': '원본 240 × 320', 'display.compact': '작게', 'display.fit': '맞춤', 'display.large': '크게', 'display.max': '최대',
  'settings.defaults.note': '이 기본값은 변경 후 새로 가져오는 게임에만 적용됩니다. 기존 게임의 설정은 유지됩니다.',
  'settings.mobile': 'iPhone 및 모바일', 'settings.keepAwake': '게임 중 화면 켜짐 유지', 'settings.keepAwake.note': '지원되는 경우 게임 실행 중 화면이 자동으로 꺼지지 않도록 합니다.',
  'settings.language': '언어', 'settings.appLanguage': '앱 언어', 'lang.en': 'English', 'lang.ko': '한국어',
  'settings.language.note': 'WIPI Player 인터페이스 언어를 변경합니다. 게임 자체의 내용은 번역하지 않습니다.', 'common.done': '완료', 'common.cancel': '취소', 'common.confirm': '확인',
  'player.display': '화면', 'player.controls': '조작', 'player.save': '세이브 관리', 'player.audio': '오디오',
  'player.customize': '🎮 조작키 사용자 설정', 'player.manageSaves': '💾 세이브 관리', 'player.effects': '🔊 효과음',
  'player.display.note': '화면 크기는 표시 크기만 변경하며 게임 내부 WIPI 프레임버퍼는 변경하지 않습니다.',
  'player.controls.note': '방향키와 숫자 키패드를 이동하고 크기를 조정할 수 있습니다. 세로/가로 레이아웃은 게임별로 따로 저장됩니다.',
  'player.save.note': '게임의 일반 세이브 데이터를 백업, 복원, 내보내기, 가져오기 또는 삭제할 수 있습니다.',
  'diagnostics.title': '진단 로그 (테스트 전용)', 'diagnostics.armExp': 'EXP 추적 시작/초기화', 'diagnostics.view': '로그 보기', 'diagnostics.export': '로그 내보내기', 'diagnostics.clear': '지우기',
  'controls.edit': '조작키 편집', 'controls.hint': '게임 화면에서 방향키 또는 숫자 키패드를 직접 드래그하세요.', 'controls.dpad': '방향키', 'controls.numpad': '숫자 키패드',
  'controls.size': '크기', 'controls.spacing': '간격', 'controls.opacity': '투명도', 'controls.starting': '기본 레이아웃', 'controls.classic': '기본', 'controls.spacious': '넓게', 'controls.compact': '좁게', 'controls.reset': '초기화', 'controls.visible': '표시할 키',
  'game.edit': '게임 편집', 'game.displayName': '표시 이름', 'game.cover': '🖼 커버 이미지 선택', 'game.saveChanges': '변경 저장',
  'actions.play': '▶ 실행', 'actions.edit': '✎ 게임 편집', 'actions.favorite': '★ 즐겨찾기 설정/해제', 'actions.display': '▣ 화면 설정', 'actions.controls': '🎮 조작키 사용자 설정', 'actions.saves': '💾 세이브 관리', 'actions.cover': '🖼 커버 변경', 'actions.export': '⇧ WIPI 항목 내보내기', 'actions.delete': '라이브러리에서 삭제',
  'saves.subtitle': '게임 내 일반 세이브 데이터', 'saves.create': '＋ 백업 만들기', 'saves.import': '백업 가져오기', 'saves.local': '로컬 백업', 'saves.stored': 'WIPI Player 내부에 저장됨', 'saves.none': '아직 백업이 없습니다.', 'saves.current': '현재 세이브 데이터', 'saves.eraseNote': '현재 게임 세이브만 삭제합니다. 위에 표시된 백업은 유지됩니다.', 'saves.erase': '현재 게임 세이브 삭제', 'saves.restore': '복원', 'saves.export': '내보내기', 'saves.delete': '삭제',
  'tutorial.library': '게임 라이브러리', 'tutorial.playing': '다시 실행하기', 'tutorial.saves': '게임 세이브', 'tutorial.keyboard': '키보드', 'tutorial.dontShow': '다시 표시하지 않기',
  'count.game': '개 게임', 'count.games': '개 게임', 'badge.portrait': '세로', 'badge.landscape': '가로',
  'sort.aToZ': '가나다/A→Z', 'sort.zToA': '역순', 'sort.oldest': '오래된 순', 'sort.newest': '최신 순', 'sort.nonFavorites': '일반 게임 먼저', 'sort.favoritesFirst': '즐겨찾기 먼저',
  'toast.favoriteAdded': '즐겨찾기에 추가했습니다.', 'toast.favoriteRemoved': '즐겨찾기에서 제거했습니다.'
};

let current: AppLanguage = 'en';
export const setLanguage = (language: AppLanguage) => { current = language; document.documentElement.lang = language === 'ko' ? 'ko' : 'en'; };
export const getLanguage = () => current;
export const t = (key: string): string => (current === 'ko' ? ko[key] : en[key]) ?? en[key] ?? key;

const setText = (selector: string, key: string) => { const el = document.querySelector<HTMLElement>(selector); if (el) el.textContent = t(key); };
const setOption = (selector: string, key: string) => { const el = document.querySelector<HTMLOptionElement>(selector); if (el) el.textContent = t(key); };
const setLabelFor = (controlId: string, key: string) => { const control = document.getElementById(controlId); const label = control?.closest('label'); const span = label?.querySelector<HTMLElement>(':scope > span'); if (span) span.textContent = t(key); };

export const applyTranslations = () => {
  setText('.app-brand p','brand.games'); setText('#library-diagnostics','header.logs'); setText('#import-game','header.import');
  setOption('#library-sort-home option[value="recent"]','sort.recent'); setOption('#library-sort-home option[value="name"]','sort.name'); setOption('#library-sort-home option[value="favorites"]','sort.favorites');
  setText('#library-empty h2','library.empty.title'); setText('#library-empty p','library.empty.body'); setText('#empty-import-game','library.empty.import');
  setText('#home-settings-title','settings.title'); setText('#home-settings-title + span','settings.subtitle');
  const hs = document.querySelectorAll<HTMLElement>('#home-settings-overlay .settings-section > strong');
  if (hs[0]) hs[0].textContent=t('settings.defaults'); if (hs[1]) hs[1].textContent=t('settings.mobile'); if (hs[2]) hs[2].textContent=t('settings.language');
  setLabelFor('home-default-orientation','settings.orientation'); setLabelFor('home-default-display','settings.screenSize');
  setOption('#home-default-orientation option[value="portrait"]','orientation.portrait'); setOption('#home-default-orientation option[value="landscape"]','orientation.landscape');
  setOption('#home-default-display option[value="native"]','display.native'); setOption('#home-default-display option[value="compact"]','display.compact'); setOption('#home-default-display option[value="fit"]','display.fit'); setOption('#home-default-display option[value="large"]','display.large'); setOption('#home-default-display option[value="max"]','display.max');
  const notes = document.querySelectorAll<HTMLElement>('#home-settings-overlay .settings-note'); if(notes[0]) notes[0].textContent=t('settings.defaults.note');
  const keep = document.querySelector<HTMLElement>('#home-keep-awake')?.closest('label')?.querySelector<HTMLElement>('span'); if(keep){ const strong=keep.querySelector('strong'); const small=keep.querySelector('small'); if(strong) strong.textContent=t('settings.keepAwake'); if(small) small.textContent=t('settings.keepAwake.note'); }
  setText('#home-language-label','settings.appLanguage'); setOption('#home-language option[value="en"]','lang.en'); setOption('#home-language option[value="ko"]','lang.ko'); setText('#home-language-note','settings.language.note'); setText('#home-settings-done','common.done');
  const ps = document.querySelectorAll<HTMLElement>('#settings-panel .settings-section > strong'); if(ps[0]) ps[0].textContent=t('player.display'); if(ps[1]) ps[1].textContent=t('player.controls'); if(ps[2]) ps[2].textContent=t('player.save'); if(ps[3]) ps[3].textContent=t('player.audio'); if(ps[4]) ps[4].textContent=t('diagnostics.title');
  setLabelFor('game-orientation','settings.orientation'); setLabelFor('game-display-mode','settings.screenSize');
  setOption('#game-orientation option[value="portrait"]','orientation.portrait'); setOption('#game-orientation option[value="landscape"]','orientation.landscape');
  setOption('#game-display-mode option[value="native"]','display.native'); setOption('#game-display-mode option[value="compact"]','display.compact'); setOption('#game-display-mode option[value="fit"]','display.fit'); setOption('#game-display-mode option[value="large"]','display.large'); setOption('#game-display-mode option[value="max"]','display.max');
  setText('#customize-controls','player.customize'); setText('#manage-saves','player.manageSaves');
  const playerNotes = document.querySelectorAll<HTMLElement>('#settings-panel .settings-note'); if(playerNotes[0]) playerNotes[0].textContent=t('player.display.note'); if(playerNotes[1]) playerNotes[1].textContent=t('player.controls.note'); if(playerNotes[2]) playerNotes[2].textContent=t('player.save.note');
  setText('#debug-arm-exp-trace','diagnostics.armExp'); setText('#debug-view-log','diagnostics.view'); setText('#debug-export-log','diagnostics.export'); setText('#debug-clear-log','diagnostics.clear');
  setText('#control-editor .control-editor-header strong','controls.edit'); setText('.control-editor-hint','controls.hint'); setText('#control-pad-direction','controls.dpad'); setText('#control-pad-number','controls.numpad'); setText('.control-editor-presets > span','controls.starting'); setText('#control-preset-classic','controls.classic'); setText('#control-preset-spacious','controls.spacious'); setText('#control-preset-compact','controls.compact'); setText('#control-reset','controls.reset'); setText('.control-key-visibility-details summary','controls.visible'); setText('#control-editor-done','common.done');
  setText('#game-editor-title','game.edit'); setText('.editor-field > span','game.displayName'); setText('#game-editor-cover','game.cover'); setText('#game-editor-cancel','common.cancel'); setText('#game-editor-save','game.saveChanges');
  setText('#action-play','actions.play'); setText('#action-edit','actions.edit'); setText('#action-favorite','actions.favorite'); setText('#action-display','actions.display'); setText('#action-controls','actions.controls'); setText('#action-saves','actions.saves'); setText('#action-cover','actions.cover'); setText('#action-export-game','actions.export'); setText('#action-delete','actions.delete');
  setText('#save-manager-title + span','saves.subtitle'); setText('#save-create-backup','saves.create'); setText('#save-import-backup','saves.import'); setText('.save-section-heading strong','saves.local'); setText('.save-section-heading span','saves.stored'); setText('#save-backup-empty','saves.none'); setText('.save-danger-zone > strong','saves.current'); setText('#save-erase-current','saves.erase');
  setText('#confirm-cancel','common.cancel');
  const th=document.querySelectorAll<HTMLElement>('.tutorial-modal h3'); if(th[0]) th[0].textContent=t('tutorial.library'); if(th[1]) th[1].textContent=t('tutorial.playing'); if(th[2]) th[2].textContent=t('tutorial.saves'); if(th[3]) th[3].textContent=t('tutorial.keyboard'); setText('#close-tutorial','common.done');
};
