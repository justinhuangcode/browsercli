/**
 * TypeScript interfaces for all browsercli RPC request/response types.
 *
 * Field names use snake_case to match the JSON wire protocol (Rust serde defaults).
 * Optional fields correspond to Rust `#[serde(default, skip_serializing_if)]`.
 */

export interface StatusResponse {
  running: boolean;
  browser_alive?: boolean;
  pid?: number;
  dir?: string;
  http_addr?: string;
  http_port?: number;
  current_url?: string;
  title?: string;
  headless?: boolean;
  browser_pid?: number;
  devtools_port?: number;
  devtools_ws_url?: string;
  browser_bin?: string;
  error?: string;
}

export interface VersionResponse {
  rpc_version: number;
  schema_version: number;
}

export interface GotoResponse {
  url: string;
  title?: string;
}

export interface EvalResponse {
  value: unknown;
}

export interface ReloadResponse {
  ok: boolean;
}

export interface DomResponse {
  selector: string;
  mode: string;
  value: string;
}

export interface DomAllResponse {
  selector: string;
  mode: string;
  values: string[];
}

export interface DomAttrResponse {
  selector: string;
  name: string;
  value: string | null;
}

export interface DomClickResponse {
  ok: boolean;
}

export interface DomTypeResponse {
  ok: boolean;
}

export interface DomWaitResponse {
  ok: boolean;
  state: string;
}

export interface ScreenshotResponse {
  format: string;
  base64: string;
}

export interface ConsoleEntry {
  level: string;
  text: string;
  timestamp: number;
}

export interface ConsoleResponse {
  entries: ConsoleEntry[];
}

export interface NetworkEntry {
  method: string;
  url: string;
  status: number;
  resource_type: string;
  mime_type: string;
  size: number;
  duration_ms: number;
  timestamp: number;
}

export interface NetworkResponse {
  entries: NetworkEntry[];
}

export interface PerfResponse {
  dom_content_loaded_ms: number;
  load_event_ms: number;
}

export interface StopResponse {
  ok: boolean;
}

export interface PluginInfo {
  name: string;
  version: string;
  description?: string;
  templates?: string[];
  hooks?: string[];
  rpc_endpoints?: string[];
}

export interface PluginListResponse {
  plugins: PluginInfo[];
}
