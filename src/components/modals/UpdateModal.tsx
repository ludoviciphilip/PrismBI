import React, { useEffect, useState } from 'react';
import { createPortal } from 'react-dom';
import {
  Download,
  RefreshCw,
  CheckCircle2,
  AlertTriangle,
  ExternalLink,
  Sparkles,
  ShieldCheck,
  X,
} from 'lucide-react';
import { useShallow } from 'zustand/react/shallow';
import { useBiStore } from '../../store/useBiStore';
import { APP_VERSION } from '../../version';
import { SimpleMarkdown } from '../shared/SimpleMarkdown';
import {
  isTauriRuntime,
  checkForUpdatesNative,
  downloadAndInstallUpdateNative,
  UpdateInstallResult,
} from '../../bridge/tauriBridge';

export const UpdateModal: React.FC = () => {
  const {
    updateModalOpen,
    setUpdateModalOpen,
    availableUpdate,
    setAvailableUpdate,
  } = useBiStore(
    useShallow(s => ({
      updateModalOpen: s.updateModalOpen,
      setUpdateModalOpen: s.setUpdateModalOpen,
      availableUpdate: s.availableUpdate,
      setAvailableUpdate: s.setAvailableUpdate,
    }))
  );

  const [checking, setChecking] = useState(false);
  const [installing, setInstalling] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [installResult, setInstallResult] = useState<UpdateInstallResult | null>(null);

  const runCheck = async () => {
    setChecking(true);
    setError(null);
    setInstallResult(null);
    try {
      if (!isTauriRuntime()) {
        setError('A verificação nativa de atualizações requer o app desktop PrismBI (Tauri). No navegador, acesse o GitHub Releases.');
        return;
      }
      const res = await checkForUpdatesNative(APP_VERSION);
      setAvailableUpdate(res);
    } catch (err: any) {
      setError(err?.message || String(err));
    } finally {
      setChecking(false);
    }
  };

  useEffect(() => {
    if (updateModalOpen && !availableUpdate && isTauriRuntime()) {
      void runCheck();
    }
  }, [updateModalOpen]);

  if (!updateModalOpen) return null;

  const handleDownloadAndInstall = async () => {
    if (!availableUpdate?.canInstall) return;
    setInstalling(true);
    setError(null);
    try {
      const res = await downloadAndInstallUpdateNative();
      setInstallResult(res);
    } catch (err: any) {
      setError(err?.message || String(err));
    } finally {
      setInstalling(false);
    }
  };

  const formatBytes = (bytes: number) => {
    if (!bytes) return '';
    return `${(bytes / (1024 * 1024)).toFixed(1)} MB`;
  };

  return createPortal(
    <div className="fixed inset-0 bg-black/60 z-[10000] flex items-center justify-center p-4">
      <div className="bg-white dark:bg-slate-900 border border-slate-200 dark:border-slate-800 rounded-2xl shadow-2xl max-w-lg w-full p-6 text-xs space-y-4">
        {/* Header */}
        <div className="flex items-center justify-between pb-3 border-b border-slate-200 dark:border-slate-800">
          <div className="flex items-center gap-2.5">
            <div className="p-2 rounded-xl bg-amber-500 text-slate-950">
              <Sparkles className="w-4 h-4" />
            </div>
            <div>
              <h3 className="font-bold text-slate-900 dark:text-slate-100 text-sm">
                Atualizações do PrismBI
              </h3>
              <p className="text-[11px] text-slate-500 dark:text-slate-400">
                Versão instalada: <strong className="font-mono">v{APP_VERSION}</strong>
              </p>
            </div>
          </div>
          <button
            type="button"
            onClick={() => setUpdateModalOpen(false)}
            className="text-slate-400 hover:text-slate-700 dark:hover:text-slate-200 p-1 cursor-pointer"
            aria-label="Fechar"
          >
            <X className="w-4 h-4" />
          </button>
        </div>

        {/* Body */}
        {checking && (
          <div className="py-8 flex flex-col items-center justify-center gap-2 text-slate-500 dark:text-slate-400">
            <RefreshCw className="w-6 h-6 text-amber-500 animate-spin" />
            <span>Consultando GitHub Releases...</span>
          </div>
        )}

        {error && (
          <div className="p-3 rounded-xl bg-rose-50 dark:bg-rose-950/40 border border-rose-200 dark:border-rose-900 text-rose-700 dark:text-rose-300 flex gap-2">
            <AlertTriangle className="w-4 h-4 shrink-0 mt-0.5" />
            <span>{error}</span>
          </div>
        )}

        {!checking && availableUpdate && (
          <div className="space-y-3">
            {availableUpdate.updateAvailable ? (
              <div className="p-3.5 rounded-xl bg-emerald-50 dark:bg-emerald-950/30 border border-emerald-200 dark:border-emerald-800 space-y-2">
                <div className="flex items-center justify-between">
                  <span className="font-bold text-emerald-800 dark:text-emerald-300 text-xs">
                    Nova versão disponível: v{availableUpdate.latestVersion}
                  </span>
                  {availableUpdate.signatureVerified && (
                    <span className="inline-flex items-center gap-1 text-[10px] text-emerald-700 dark:text-emerald-400 font-medium">
                      <ShieldCheck className="w-3 h-3" /> Assinatura verificada
                    </span>
                  )}
                </div>
                <div className="text-[11px] text-slate-600 dark:text-slate-300 flex items-center justify-between">
                  <span className="font-mono truncate">{availableUpdate.assetName || 'GitHub Release'}</span>
                  {availableUpdate.assetSize > 0 && (
                    <span className="font-mono text-slate-500 shrink-0 ml-2">
                      {formatBytes(availableUpdate.assetSize)}
                    </span>
                  )}
                </div>
                {!availableUpdate.canInstall && availableUpdate.installBlockedReason && (
                  <div className="flex gap-1.5 text-[11px] text-amber-800 dark:text-amber-300">
                    <AlertTriangle className="w-3.5 h-3.5 shrink-0 mt-0.5" />
                    <span>{availableUpdate.installBlockedReason}</span>
                  </div>
                )}
                {availableUpdate.releaseNotes && (
                  <SimpleMarkdown
                    source={availableUpdate.releaseNotes}
                    className="mt-2 p-2.5 rounded-lg bg-white/80 dark:bg-slate-950/70 border border-slate-200 dark:border-slate-800 text-[11px] text-slate-700 dark:text-slate-300 max-h-56 overflow-y-auto"
                  />
                )}
              </div>
            ) : (
              <div className="p-4 rounded-xl bg-slate-50 dark:bg-slate-800/60 border border-slate-200 dark:border-slate-700 flex items-center gap-3">
                <CheckCircle2 className="w-5 h-5 text-emerald-500 shrink-0" />
                <div>
                  <div className="font-bold text-slate-900 dark:text-slate-100">
                    Você já está na versão mais recente!
                  </div>
                  <div className="text-[11px] text-slate-500 dark:text-slate-400">
                    Última release publicada: v{availableUpdate.latestVersion}
                  </div>
                </div>
              </div>
            )}
          </div>
        )}

        {installResult && (
          <div className="p-3.5 rounded-xl bg-emerald-50 dark:bg-emerald-950/40 border border-emerald-300 dark:border-emerald-800 text-emerald-800 dark:text-emerald-200 space-y-1">
            <div className="font-bold flex items-center gap-1.5">
              <CheckCircle2 className="w-4 h-4" />
              <span>{installResult.installed ? 'Atualização instalada!' : 'Pacote salvo em Downloads!'}</span>
            </div>
            <p className="text-[11px]">{installResult.message}</p>
          </div>
        )}

        {/* Footer */}
        <div className="pt-3 border-t border-slate-200 dark:border-slate-800 flex items-center justify-between gap-2">
          <a
            href={availableUpdate?.releaseUrl || 'https://github.com/ludoviciphilip/PrismBI/releases'}
            target="_blank"
            rel="noreferrer"
            className="inline-flex items-center gap-1 text-slate-500 hover:text-amber-600 dark:text-slate-400 dark:hover:text-amber-400 font-medium"
          >
            <ExternalLink className="w-3.5 h-3.5" />
            <span>Ver no GitHub</span>
          </a>

          <div className="flex items-center gap-2">
            <button
              type="button"
              onClick={() => void runCheck()}
              disabled={checking || installing}
              className="px-3 py-1.5 rounded-lg bg-slate-100 dark:bg-slate-800 hover:bg-slate-200 dark:hover:bg-slate-700 text-slate-700 dark:text-slate-200 font-semibold flex items-center gap-1.5 cursor-pointer disabled:opacity-50"
            >
              <RefreshCw className={`w-3.5 h-3.5 ${checking ? 'animate-spin' : ''}`} />
              <span>Verificar</span>
            </button>

            {availableUpdate?.updateAvailable && availableUpdate.canInstall && !installResult && (
              <button
                type="button"
                onClick={() => void handleDownloadAndInstall()}
                disabled={installing}
                className="px-4 py-1.5 rounded-lg bg-amber-500 hover:bg-amber-400 text-slate-950 font-bold flex items-center gap-1.5 cursor-pointer disabled:opacity-60 shadow-xs"
              >
                <Download className="w-3.5 h-3.5" />
                <span>
                  {installing
                    ? 'Baixando...'
                    : availableUpdate.isAppimage
                      ? 'Baixar e Atualizar AppImage'
                      : availableUpdate.installKind === 'msiInstalled'
                        ? 'Baixar e Instalar (.msi)'
                        : 'Baixar Pacote'}
                </span>
              </button>
            )}
          </div>
        </div>
      </div>
    </div>,
    document.body
  );
};
