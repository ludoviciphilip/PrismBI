/**
 * @license
 * SPDX-License-Identifier: Apache-2.0
 */

import React, { useEffect, useState } from 'react';
import { CheckCircle2, AlertCircle, Info } from 'lucide-react';
import { subscribeShortcutToast, ShortcutToastMessage } from '../../services/shortcuts/toastBus';

/**
 * Toast global para confirmações de atalhos de teclado (ex.: "Projeto
 * salvo" no Ctrl+S). Montado uma única vez em App.tsx.
 */
export const ShortcutToast: React.FC = () => {
  const [msg, setMsg] = useState<ShortcutToastMessage | null>(null);

  useEffect(() => {
    const unsubscribe = subscribeShortcutToast(next => setMsg(next));
    return unsubscribe;
  }, []);

  useEffect(() => {
    if (!msg) return;
    const timer = setTimeout(() => setMsg(null), 3000);
    return () => clearTimeout(timer);
  }, [msg]);

  if (!msg) return null;

  const colorClass =
    msg.kind === 'error'
      ? 'bg-red-600 text-white'
      : msg.kind === 'info'
        ? 'bg-slate-800 text-white'
        : 'bg-emerald-600 text-white';

  const Icon = msg.kind === 'error' ? AlertCircle : msg.kind === 'info' ? Info : CheckCircle2;

  return (
    <div
      key={msg.id}
      data-testid="shortcut-toast"
      role="status"
      className={`fixed bottom-5 right-5 z-[10050] flex items-center gap-2 px-4 py-2.5 rounded-lg shadow-2xl text-xs font-semibold animate-in fade-in slide-in-from-bottom-2 ${colorClass}`}
    >
      <Icon className="w-4 h-4" />
      <span>{msg.text}</span>
    </div>
  );
};
