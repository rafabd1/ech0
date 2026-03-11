interface WipeConfirmDialogProps {
  onConfirm: () => void;
  onCancel: () => void;
}

export default function WipeConfirmDialog({ onConfirm, onCancel }: WipeConfirmDialogProps) {
  return (
    <div
      className="absolute inset-0 z-50 bg-black/80 backdrop-blur-sm flex items-center justify-center px-6"
      onClick={onCancel}
    >
      <div
        className="w-full max-w-xs bg-surface border border-border rounded-xl p-6 flex flex-col gap-5"
        onClick={(e) => e.stopPropagation()}
      >
        <div className="flex flex-col gap-2">
          <p className="text-sm font-mono text-white uppercase tracking-widest">
            confirm wipe
          </p>
          <p className="text-[11px] text-muted leading-relaxed">
            all messages, session keys and identity material will be permanently
            destroyed. this cannot be undone.
          </p>
        </div>

        <div className="flex gap-3">
          <button
            onClick={onCancel}
            className="flex-1 py-2.5 border border-border rounded text-[11px] font-mono text-muted hover:text-white hover:border-secondary transition-colors uppercase tracking-wider"
          >
            cancel
          </button>
          <button
            onClick={onConfirm}
            className="flex-1 py-2.5 border border-white rounded text-[11px] font-mono text-white hover:bg-white hover:text-black transition-colors uppercase tracking-wider"
          >
            wipe
          </button>
        </div>
      </div>
    </div>
  );
}
