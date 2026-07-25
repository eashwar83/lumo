// Curated flagship / most-recent vision models per provider — the pre-populated
// dropdown. "Fetch latest models" (see AiSettings) augments these with anything
// newly released that the provider's API reports.
//
// These are best-effort defaults; the fetch button is the source of truth for
// what's actually available on a given key.

export type AiProvider =
    | "Gemini"
    | "Claude"
    | "OpenAI"
    | "Grok (xAI)"
    | "Kimi (Moonshot)"
    | "Qwen"
    | "DeepSeek"
    | "Custom (OpenAI-compatible)";

export const AI_PROVIDERS: AiProvider[] = [
    "Gemini",
    "Claude",
    "OpenAI",
    "Grok (xAI)",
    "Kimi (Moonshot)",
    "Qwen",
    "DeepSeek",
    "Custom (OpenAI-compatible)",
];

// First entry is the default for that provider.
// The FIRST entry is the default and must match the backend's fallback model
// for that provider (see ai_enhance.rs provider_config), so a blank model and
// the shown default never disagree.
export const CURATED_AI_MODELS: Record<AiProvider, string[]> = {
    Gemini: [
        "gemini-2.0-flash",
        "gemini-2.5-pro",
        "gemini-2.5-flash",
        "gemini-2.0-flash-lite",
        "gemini-1.5-pro",
    ],
    Claude: [
        "claude-sonnet-5",
        "claude-opus-4-8",
        "claude-haiku-4-5-20251001",
        "claude-fable-5",
    ],
    OpenAI: ["gpt-4o", "gpt-4.1", "gpt-4.1-mini", "gpt-4o-mini", "o4-mini"],
    "Grok (xAI)": [
        "grok-2-vision-1212",
        "grok-4",
        "grok-3",
        "grok-2-vision",
    ],
    "Kimi (Moonshot)": [
        "moonshot-v1-8k-vision-preview",
        "moonshot-v1-32k-vision-preview",
        "moonshot-v1-128k-vision-preview",
        "kimi-latest",
    ],
    Qwen: [
        "qwen-vl-plus",
        "qwen-vl-max",
        "qwen2.5-vl-72b-instruct",
        "qwen2.5-vl-7b-instruct",
    ],
    DeepSeek: ["deepseek-chat", "deepseek-reasoner"],
    "Custom (OpenAI-compatible)": [],
};

export const defaultModelFor = (provider: string): string =>
    CURATED_AI_MODELS[provider as AiProvider]?.[0] ?? "";

// Default OpenAI-compatible API roots, shown as the Base URL placeholder. A user
// can override per provider for a regional / workspace / self-hosted endpoint.
export const DEFAULT_AI_BASE_URLS: Record<AiProvider, string> = {
    Gemini: "https://generativelanguage.googleapis.com/v1beta/openai",
    Claude: "https://api.anthropic.com/v1",
    OpenAI: "https://api.openai.com/v1",
    "Grok (xAI)": "https://api.x.ai/v1",
    "Kimi (Moonshot)": "https://api.moonshot.ai/v1",
    Qwen: "https://dashscope.aliyuncs.com/compatible-mode/v1",
    DeepSeek: "https://api.deepseek.com/v1",
    "Custom (OpenAI-compatible)": "",
};

export const defaultBaseUrlFor = (provider: string): string =>
    DEFAULT_AI_BASE_URLS[provider as AiProvider] ?? "";
