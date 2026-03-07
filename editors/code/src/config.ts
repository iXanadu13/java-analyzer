import * as vscode from "vscode";

export const CONFIG_NAMESPACE = "java-analyzer";
export const EXTENSION_ID = "java-analyzer";

export const EXTENSION_CONFIG_KEYS = {
  jdkPath: "jdkPath",
  decompilerBackend: "decompilerBackend",
  vineflowerPath: "vineflowerPath",
  cfrPath: "cfrPath",
  legacyDecompilerPath: "decompilerPath",
  serverPath: "serverPath",
} as const;

const RELEVANT_CONFIGURATION_PATHS = [
  EXTENSION_CONFIG_KEYS.jdkPath,
  EXTENSION_CONFIG_KEYS.decompilerBackend,
  EXTENSION_CONFIG_KEYS.vineflowerPath,
  EXTENSION_CONFIG_KEYS.cfrPath,
  EXTENSION_CONFIG_KEYS.serverPath,
].map((key) => `${CONFIG_NAMESPACE}.${key}`);

export type ExtensionConfigKey =
  (typeof EXTENSION_CONFIG_KEYS)[keyof typeof EXTENSION_CONFIG_KEYS];

export type DecompilerBackend = "vineflower" | "cfr";

export interface ExtensionSettings {
  jdkPath: string;
  decompilerBackend: DecompilerBackend;
  vineflowerPath: string;
  cfrPath: string;
  serverPath: string;
}

export function getExtensionSettings(): ExtensionSettings {
  const config = vscode.workspace.getConfiguration(CONFIG_NAMESPACE);
  return {
    jdkPath: config.get<string>(EXTENSION_CONFIG_KEYS.jdkPath, "").trim(),
    decompilerBackend: normalizeDecompilerBackend(
      config.get<string>(EXTENSION_CONFIG_KEYS.decompilerBackend),
    ),
    vineflowerPath: config.get<string>(EXTENSION_CONFIG_KEYS.vineflowerPath, "").trim(),
    cfrPath: config.get<string>(EXTENSION_CONFIG_KEYS.cfrPath, "").trim(),
    serverPath: config.get<string>(EXTENSION_CONFIG_KEYS.serverPath, "").trim(),
  };
}

export function didRelevantConfigChange(event: vscode.ConfigurationChangeEvent): boolean {
  return RELEVANT_CONFIGURATION_PATHS.some((path) => event.affectsConfiguration(path));
}

export function updateConfigurationValue(
  key: ExtensionConfigKey,
  value: string,
): Thenable<void> {
  return vscode.workspace
    .getConfiguration(CONFIG_NAMESPACE)
    .update(key, value, vscode.ConfigurationTarget.Global);
}

export function normalizeOptionalPath(value: string): string | undefined {
  const trimmed = value.trim();
  if (!trimmed) {
    return undefined;
  }
  return trimmed;
}

export async function migrateLegacyDecompilerPathIfNeeded(): Promise<void> {
  const settings = getExtensionSettings();
  const config = vscode.workspace.getConfiguration(CONFIG_NAMESPACE);
  const legacyPath = config.get<string>(EXTENSION_CONFIG_KEYS.legacyDecompilerPath, "").trim();

  if (!legacyPath) {
    return;
  }

  if (settings.decompilerBackend === "cfr" && !settings.cfrPath) {
    await updateConfigurationValue(EXTENSION_CONFIG_KEYS.cfrPath, legacyPath);
    return;
  }

  if (settings.decompilerBackend === "vineflower" && !settings.vineflowerPath) {
    await updateConfigurationValue(EXTENSION_CONFIG_KEYS.vineflowerPath, legacyPath);
  }
}

function normalizeDecompilerBackend(value: string | undefined): DecompilerBackend {
  if (value === "cfr") {
    return value;
  }
  return "vineflower";
}
