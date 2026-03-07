import * as vscode from "vscode";

import {
  type DecompilerBackend,
  normalizeOptionalPath,
  type ExtensionSettings,
} from "./config";

function resolveBundledDecompilerPath(
  context: vscode.ExtensionContext,
  backend: DecompilerBackend,
): string {
  const fileName = backend === "cfr" ? "cfr.jar" : "vineflower.jar";
  return vscode.Uri.joinPath(
    context.extensionUri,
    "resources",
    "decompilers",
    fileName,
  ).fsPath;
}

function getBackendCustomPath(settings: ExtensionSettings): string | undefined {
  if (settings.decompilerBackend === "cfr") {
    return normalizeOptionalPath(settings.cfrPath);
  }
  return normalizeOptionalPath(settings.vineflowerPath);
}

export function resolveEffectiveDecompilerPath(
  context: vscode.ExtensionContext,
  settings: ExtensionSettings,
): string {
  return (
    getBackendCustomPath(settings)
    ?? resolveBundledDecompilerPath(context, settings.decompilerBackend)
  );
}
