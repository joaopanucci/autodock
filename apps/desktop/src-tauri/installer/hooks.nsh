; Hooks do instalador NSIS do AutoDock.
; Depois de instalar/atualizar, avisa o Explorer que os ícones mudaram. Sem isso o
; Windows continua exibindo o ícone antigo nos atalhos (cache de ícones por caminho).
!macro NSIS_HOOK_POSTINSTALL
  ; SHChangeNotify(SHCNE_ASSOCCHANGED = 0x08000000, SHCNF_IDLIST = 0, NULL, NULL)
  System::Call 'shell32::SHChangeNotify(i 0x08000000, i 0, p 0, p 0)'
!macroend
