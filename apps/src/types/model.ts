export interface ModelReasoningLevel {
  effort: string;
  description: string;
  [key: string]: unknown;
}

export interface ModelTruncationPolicy {
  mode: string;
  limit: number;
  [key: string]: unknown;
}

export interface ModelInfo {
  slug: string;
  displayName: string;
  description: string | null;
  defaultReasoningLevel: string | null;
  supportedReasoningLevels: ModelReasoningLevel[];
  shellType: string | null;
  visibility: string | null;
  supportedInApi: boolean;
  priority: number;
  additionalSpeedTiers: string[];
  availabilityNux: Record<string, unknown> | null;
  upgrade: Record<string, unknown> | null;
  baseInstructions: string | null;
  modelMessages: Record<string, unknown> | null;
  supportsReasoningSummaries: boolean | null;
  defaultReasoningSummary: string | null;
  supportVerbosity: boolean | null;
  defaultVerbosity: unknown | null;
  applyPatchToolType: string | null;
  webSearchToolType: string | null;
  truncationPolicy: ModelTruncationPolicy | null;
  supportsParallelToolCalls: boolean | null;
  supportsImageDetailOriginal: boolean | null;
  contextWindow: number | null;
  autoCompactTokenLimit: number | null;
  effectiveContextWindowPercent: number | null;
  experimentalSupportedTools: string[];
  inputModalities: string[];
  minimalClientVersion: unknown | null;
  supportsSearchTool: boolean | null;
  availableInPlans: string[];
  [key: string]: unknown;
}

export interface ModelCatalog {
  models: ModelInfo[];
  [key: string]: unknown;
}

export interface ManagedModelInfo extends ModelInfo {
  sourceKind: string;
  userEdited: boolean;
  sortIndex: number;
  updatedAt: number;
}

export interface ManagedModelCatalog {
  items: ManagedModelInfo[];
  [key: string]: unknown;
}
