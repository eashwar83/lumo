<script setup lang="ts">
import { computed, nextTick, onBeforeUnmount, ref, watch } from "vue";

const props = withDefaults(
    defineProps<{
        open: boolean;
        title: string;
        message: string;
        confirmText?: string;
        cancelText?: string;
        confirmLoading?: boolean;
        errorMessage?: string;
    }>(),
    {
        confirmText: "Confirm",
        cancelText: "Cancel",
        confirmLoading: false,
        errorMessage: "",
    },
);

const emit = defineEmits<{
    (e: "cancel"): void;
    (e: "confirm"): void;
}>();

const modalRef = ref<HTMLElement | null>(null);
const cancelBtnRef = ref<HTMLButtonElement | null>(null);
const confirmBtnRef = ref<HTMLButtonElement | null>(null);
const previousFocusedElement = ref<HTMLElement | null>(null);
const titleId = `confirm-dialog-title-${Math.random().toString(36).slice(2, 10)}`;
const hasError = computed(() => Boolean(props.errorMessage));

const getFocusableElements = () => {
    const modal = modalRef.value;
    if (!modal) return [] as HTMLElement[];
    return Array.from(
        modal.querySelectorAll<HTMLElement>(
            'button:not([disabled]), [href], input:not([disabled]), select:not([disabled]), textarea:not([disabled]), [tabindex]:not([tabindex="-1"])',
        ),
    );
};

const focusPrimaryButton = () => {
    (cancelBtnRef.value || confirmBtnRef.value)?.focus();
};

const onBackdropClick = () => {
    if (props.confirmLoading) return;
    emit("cancel");
};

const onCancelClick = () => {
    if (props.confirmLoading) return;
    emit("cancel");
};

const onKeydown = (event: KeyboardEvent) => {
    if (event.key === "Escape") {
        event.preventDefault();
        onCancelClick();
        return;
    }
    if (event.key !== "Tab") return;

    const focusable = getFocusableElements();
    if (!focusable.length) {
        event.preventDefault();
        return;
    }

    const first = focusable[0];
    const last = focusable[focusable.length - 1];
    const active = document.activeElement as HTMLElement | null;
    const insideModal = !!(active && modalRef.value?.contains(active));

    if (event.shiftKey) {
        if (!insideModal || active === first) {
            event.preventDefault();
            last.focus();
        }
        return;
    }

    if (!insideModal || active === last) {
        event.preventDefault();
        first.focus();
    }
};

const onDocumentFocusIn = (event: FocusEvent) => {
    if (!props.open) return;
    const target = event.target as HTMLElement | null;
    if (!target) return;
    if (modalRef.value?.contains(target)) return;
    focusPrimaryButton();
};

watch(
    () => props.open,
    async (open) => {
        if (open) {
            previousFocusedElement.value =
                document.activeElement instanceof HTMLElement
                    ? document.activeElement
                    : null;
            document.addEventListener("focusin", onDocumentFocusIn);
            await nextTick();
            focusPrimaryButton();
            return;
        }

        document.removeEventListener("focusin", onDocumentFocusIn);
        const previous = previousFocusedElement.value;
        previousFocusedElement.value = null;
        if (previous) {
            await nextTick();
            previous.focus();
        }
    },
);

onBeforeUnmount(() => {
    document.removeEventListener("focusin", onDocumentFocusIn);
});
</script>

<template>
    <Teleport to="body">
        <div v-if="props.open" class="confirm-dialog" @keydown="onKeydown">
            <div class="confirm-dialog__backdrop" @click="onBackdropClick"></div>
            <div
                ref="modalRef"
                class="confirm-dialog__panel"
                role="dialog"
                aria-modal="true"
                :aria-labelledby="titleId"
                tabindex="-1"
            >
                <div :id="titleId" class="confirm-dialog__title">
                    {{ props.title }}
                </div>
                <div class="confirm-dialog__body">
                    {{ props.message }}
                </div>
                <div v-if="hasError" class="confirm-dialog__error">
                    {{ props.errorMessage }}
                </div>
                <div class="confirm-dialog__actions">
                    <button
                        ref="cancelBtnRef"
                        class="confirm-dialog__btn confirm-dialog__btn--ghost"
                        type="button"
                        :disabled="props.confirmLoading"
                        @click="onCancelClick"
                    >
                        {{ props.cancelText }}
                    </button>
                    <button
                        ref="confirmBtnRef"
                        class="confirm-dialog__btn confirm-dialog__btn--danger"
                        type="button"
                        :disabled="props.confirmLoading"
                        @click="emit('confirm')"
                    >
                        {{ props.confirmText }}
                    </button>
                </div>
            </div>
        </div>
    </Teleport>
</template>

<style scoped>
.confirm-dialog {
    position: fixed;
    inset: 0;
    z-index: 220;
}

.confirm-dialog__backdrop {
    position: absolute;
    inset: 0;
    background: rgba(0, 0, 0, 0.62);
    backdrop-filter: blur(2px);
}

.confirm-dialog__panel {
    position: absolute;
    top: 50%;
    left: 50%;
    transform: translate(-50%, -50%);
    width: min(440px, calc(100% - 40px));
    max-height: calc(100% - 40px);
    overflow: auto;
    border-radius: 16px;
    border: 1px solid rgba(255, 255, 255, 0.08);
    background: rgba(26, 26, 26, 0.92);
    color: #f6f6f6;
    padding: 18px;
    box-shadow:
        0 12px 28px rgba(0, 0, 0, 0.35),
        inset 0 1px 0 rgba(255, 255, 255, 0.06);
}

.confirm-dialog__title {
    font-size: 16px;
    font-weight: 600;
    margin-bottom: 14px;
}

.confirm-dialog__body {
    font-size: 13px;
    line-height: 1.5;
    color: rgba(255, 255, 255, 0.82);
}

.confirm-dialog__error {
    margin-top: 12px;
    font-size: 12px;
    color: #c13838;
}

.confirm-dialog__actions {
    margin-top: 16px;
    display: flex;
    justify-content: flex-end;
    gap: 8px;
    padding-top: 12px;
    border-top: 1px solid rgba(255, 255, 255, 0.08);
}

.confirm-dialog__btn {
    min-width: 96px;
    min-height: 34px;
    border-radius: 10px;
    padding: 8px 12px;
    font-size: 12px;
    font-weight: 600;
    border: 1px solid rgba(255, 255, 255, 0.16);
    background: rgba(255, 255, 255, 0.08);
    color: #f6f6f6;
    cursor: pointer;
}

.confirm-dialog__btn:disabled {
    opacity: 0.65;
    cursor: default;
}

.confirm-dialog__btn--ghost {
    background: transparent;
}

.confirm-dialog__btn--danger {
    border-color: rgba(195, 78, 78, 0.55);
    background: rgba(195, 78, 78, 0.2);
    color: #ffd8d8;
}

.confirm-dialog__btn--danger:hover {
    border-color: rgba(225, 104, 104, 0.75);
    background: rgba(205, 72, 72, 0.34);
}

:global(:root[data-theme="graphite"] .confirm-dialog__backdrop) {
    background: rgba(10, 15, 20, 0.74);
}

:global(:root[data-theme="graphite"] .confirm-dialog__panel) {
    border-color: rgba(146, 158, 175, 0.34);
    background: rgba(37, 42, 48, 0.98);
    color: #edf1f6;
    box-shadow:
        0 24px 42px rgba(0, 0, 0, 0.48),
        inset 0 1px 0 rgba(188, 196, 208, 0.08);
}

:global(:root[data-theme="graphite"] .confirm-dialog__title) {
    color: #f3f6fa;
}

:global(:root[data-theme="graphite"] .confirm-dialog__body) {
    color: rgba(220, 226, 234, 0.9);
}

:global(:root[data-theme="graphite"] .confirm-dialog__actions) {
    border-top-color: rgba(150, 162, 178, 0.2);
}

:global(:root[data-theme="graphite"] .confirm-dialog__btn) {
    border-color: rgba(148, 161, 179, 0.34);
    background: rgba(118, 130, 146, 0.18);
    color: #f1f4f8;
}

:global(:root[data-theme="graphite"] .confirm-dialog__btn--ghost) {
    background: transparent;
}

:global(:root[data-theme="graphite"] .confirm-dialog__btn--danger) {
    border-color: rgba(212, 105, 105, 0.72);
    background: rgba(186, 89, 89, 0.3);
    color: #ffe3e3;
}

:global(:root[data-theme="graphite"] .confirm-dialog__btn--danger:hover) {
    border-color: rgba(228, 126, 126, 0.86);
    background: rgba(198, 92, 92, 0.44);
}

:global(:root[data-theme="light"] .confirm-dialog__backdrop) {
    background: rgba(245, 248, 252, 0.5);
}

:global(:root[data-theme="light"] .confirm-dialog__panel) {
    border-color: rgba(0, 0, 0, 0.12);
    background: rgba(255, 255, 255, 0.95);
    color: rgba(26, 36, 48, 0.92);
    box-shadow:
        0 14px 30px rgba(0, 0, 0, 0.14),
        inset 0 1px 0 rgba(255, 255, 255, 0.72);
}

:global(:root[data-theme="light"] .confirm-dialog__body) {
    color: rgba(33, 45, 60, 0.82);
}

:global(:root[data-theme="light"] .confirm-dialog__actions) {
    border-top-color: rgba(0, 0, 0, 0.08);
}

:global(:root[data-theme="light"] .confirm-dialog__btn) {
    border-color: rgba(0, 0, 0, 0.14);
    background: rgba(0, 0, 0, 0.04);
    color: rgba(28, 38, 52, 0.9);
}

:global(:root[data-theme="light"] .confirm-dialog__btn--ghost) {
    background: transparent;
}

:global(:root[data-theme="light"] .confirm-dialog__btn--danger) {
    border-color: rgba(195, 78, 78, 0.5);
    background: rgba(195, 78, 78, 0.12);
    color: rgba(145, 38, 38, 0.92);
}

:global(:root[data-theme="light"] .confirm-dialog__btn--danger:hover) {
    border-color: rgba(205, 84, 84, 0.62);
    background: rgba(205, 84, 84, 0.18);
}
</style>
