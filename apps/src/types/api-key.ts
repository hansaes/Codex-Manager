export interface ApiKey {
  id: string;
  name: string;
  model: string;
  modelSlug: string;
  reasoningEffort: string;
  serviceTier: string;
  rotationStrategy: string;
  aggregateApiId: string | null;
  accountPlanFilter: string | null;
  aggregateApiUrl: string | null;
  protocol: string;
  clientType: string;
  authScheme: string;
  upstreamBaseUrl: string;
  staticHeadersJson: string;
  totalTokenLimit: number | null;
  totalCostUsdLimit: number | null;
  totalRequestLimit: number | null;
  status: string;
  createdAt: number | null;
  lastUsedAt: number | null;
}

export interface ApiKeyCreateResult {
  id: string;
  key: string;
}

export interface AggregateApi {
  id: string;
  providerType: string;
  supplierName: string | null;
  sort: number;
  url: string;
  authType: string;
  authParams: Record<string, unknown> | null;
  action: string | null;
  upstreamFormat: string;
  modelsPath: string | null;
  responsesPath: string | null;
  chatCompletionsPath: string | null;
  proxyMode: string;
  proxyUrl: string | null;
  status: string;
  createdAt: number | null;
  updatedAt: number | null;
  lastTestAt: number | null;
  lastTestStatus: string | null;
  lastTestError: string | null;
  modelsLastSyncedAt: number | null;
  modelsLastSyncStatus: string | null;
  modelsLastSyncError: string | null;
}

export interface AggregateApiCreateResult {
  id: string;
  key: string;
}

export interface AggregateApiSecretResult {
  id: string;
  key: string;
  authType: string;
  username: string | null;
  password: string | null;
}

export interface AggregateApiTestResult {
  id: string;
  ok: boolean;
  statusCode: number | null;
  message: string | null;
  testedAt: number;
  latencyMs: number;
}

export interface AggregateApiModel {
  aggregateApiId: string;
  modelSlug: string;
  displayName: string | null;
  updatedAt: number | null;
}

export interface AggregateApiFetchedModel extends AggregateApiModel {
  rawJson: string | null;
}

export interface AggregateApiFetchModelsResult {
  id: string;
  count: number;
  fetchedAt: number;
  items: AggregateApiFetchedModel[];
}

export interface AggregateApiSaveModelsResult {
  id: string;
  count: number;
  syncedAt: number;
}

export interface ApiKeyUsageStat {
  keyId: string;
  requestCount: number;
  totalTokens: number;
  estimatedCostUsd: number;
}
