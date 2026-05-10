<script setup lang="ts">
type ProtocolOption = {
    value: string;
    label: string;
};

type ConnectionForm = {
    label: string;
    protocol: string;
    baseUrl: string;
    host: string;
    share: string;
    group: string;
    port: string;
    username: string;
    password: string;
    defaultPath: string;
};

const props = defineProps<{
    open: boolean;
    isEditingConnection: boolean;
    selectedProtocolLabel: string;
    protocolOptions: ProtocolOption[];
    createForm: ConnectionForm;
    isSmbProtocol: boolean;
    isFtpProtocol: boolean;
    requiresAuthFields: boolean;
    serverFieldLabel: string;
    serverFieldPlaceholder: string;
    defaultPathLabel: string;
    createError: string;
    isCreatingConnection: boolean;
}>();

const emit = defineEmits<{
    (e: "close"): void;
    (e: "submit"): void;
}>();
</script>

<template>
    <div
        v-if="props.open"
        class="network-modal__backdrop"
        @click="emit('close')"
    ></div>
    <div v-if="props.open" class="network-modal ui-surface">
        <div class="network-modal__title">
            {{
                props.isEditingConnection
                    ? `Edit ${props.selectedProtocolLabel} Connection`
                    : `Add ${props.selectedProtocolLabel} Connection`
            }}
        </div>
        <div class="network-modal__form">
            <label class="network-modal__field">
                <span>Protocol</span>
                <div class="panel__select-wrap">
                    <select
                        v-model="props.createForm.protocol"
                        class="panel__select panel__select--card network-modal__input network-modal__select"
                    >
                        <option
                            v-for="option in props.protocolOptions"
                            :key="option.value"
                            :value="option.value"
                        >
                            {{ option.label }}
                        </option>
                    </select>
                </div>
            </label>
            <label class="network-modal__field">
                <span>Name</span>
                <input
                    v-model="props.createForm.label"
                    class="panel__input panel__input--path network-modal__input"
                    type="text"
                    placeholder="Optional (auto fill)"
                />
            </label>

            <template v-if="props.isSmbProtocol">
                <label class="network-modal__field">
                    <span>Host</span>
                    <input
                        v-model="props.createForm.host"
                        class="panel__input panel__input--path network-modal__input"
                        type="text"
                        placeholder="192.168.31.33"
                    />
                </label>
                <label class="network-modal__field">
                    <span>Share (optional)</span>
                    <input
                        v-model="props.createForm.share"
                        class="panel__input panel__input--path network-modal__input"
                        type="text"
                        placeholder="Leave empty to browse shares"
                    />
                </label>
            </template>

            <template v-else-if="props.isFtpProtocol">
                <label class="network-modal__field">
                    <span>Host</span>
                    <input
                        v-model="props.createForm.host"
                        class="panel__input panel__input--path network-modal__input"
                        type="text"
                        placeholder="192.168.31.50"
                    />
                </label>
                <label class="network-modal__field">
                    <span>Port</span>
                    <input
                        v-model="props.createForm.port"
                        class="panel__input panel__input--path network-modal__input"
                        type="text"
                        inputmode="numeric"
                        placeholder="21"
                    />
                </label>
            </template>

            <template v-else>
                <label class="network-modal__field">
                    <span>{{ props.serverFieldLabel }}</span>
                    <input
                        v-model="props.createForm.baseUrl"
                        class="panel__input panel__input--path network-modal__input"
                        type="text"
                        :placeholder="props.serverFieldPlaceholder"
                    />
                </label>
            </template>

            <template v-if="props.requiresAuthFields">
                <label class="network-modal__field">
                    <span>Username</span>
                    <input
                        v-model="props.createForm.username"
                        class="panel__input panel__input--path network-modal__input"
                        type="text"
                        placeholder="Optional"
                    />
                </label>
                <label class="network-modal__field">
                    <span>Password</span>
                    <input
                        v-model="props.createForm.password"
                        class="panel__input panel__input--path network-modal__input"
                        type="password"
                        placeholder="Optional"
                    />
                </label>
                <label
                    v-if="props.isSmbProtocol"
                    class="network-modal__field"
                >
                    <span>Group</span>
                    <input
                        v-model="props.createForm.group"
                        class="panel__input panel__input--path network-modal__input"
                        type="text"
                        placeholder="Optional"
                    />
                </label>
            </template>
            <label class="network-modal__field">
                <span>{{ props.defaultPathLabel }}</span>
                <input
                    v-model="props.createForm.defaultPath"
                    class="panel__input panel__input--path network-modal__input"
                    type="text"
                    placeholder="/"
                />
            </label>
        </div>
        <div v-if="props.createError" class="network-modal__error">
            {{ props.createError }}
        </div>
        <div class="network-modal__actions">
            <button
                class="panel__action panel__action--ghost network-modal__btn"
                type="button"
                :disabled="props.isCreatingConnection"
                @click="emit('close')"
            >
                Cancel
            </button>
            <button
                class="panel__action network-modal__btn"
                type="button"
                :disabled="props.isCreatingConnection"
                @click="emit('submit')"
            >
                {{
                    props.isCreatingConnection
                        ? "Saving..."
                        : props.isEditingConnection
                          ? "Save"
                          : "Create"
                }}
            </button>
        </div>
    </div>
</template>
