// Realistic mock view-model for visual QA (mirrors the Rust ViewModel shape).
const DEMO_VM = {
  overview: {
    concurrentSessions: 3,
    todayRequests: 1842,
    todayCost: 12.4567,
    avgResponseTime: 1850,
    todayErrorRate: 0.021,
    recentMinuteRequests: 7,
    yesterdaySamePeriodRequests: 1520,
    yesterdaySamePeriodCost: 10.2,
  },
  activeSessions: [
    { providerName: "Anthropic Direct", model: "claude-sonnet-4", userName: "alice", totalTokens: 184200, costUsd: 0.842, requestCount: 12 },
    { providerName: "OpenAI Relay", model: "gpt-4o", userName: "bob", totalTokens: 92100, costUsd: 0.412, requestCount: 7 },
  ],
  leaderboard: [
    { id: "u1", title: "alice", subtitle: "user", requests: 820, cost: 6.21, tokens: 1840000, inputTokens: 920000, cacheHitRateOverride: 0.78 },
    { id: "u2", title: "bob", subtitle: "user", requests: 540, cost: 3.84, tokens: 1120000, inputTokens: 610000, cacheHitRateOverride: 0.66 },
    { id: "u3", title: "carol", subtitle: "user", requests: 312, cost: 1.92, tokens: 540000, inputTokens: 280000, cacheHitRateOverride: 0.54 },
    { id: "u4", title: "dave", subtitle: "user", requests: 170, cost: 0.51, tokens: 210000, inputTokens: 120000, cacheHitRateOverride: 0.41 },
  ],
  leaderboardSummary: { requests: 1842, cost: 12.48, tokens: 3710000, cacheHitRate: 0.69 },
  logs: [
    { id: 5001, createdAt: new Date(Date.now() - 12000).toISOString(), providerName: "Anthropic Direct", userName: "alice", model: "claude-sonnet-4", statusCode: 200, inputTokens: 42000, outputTokens: 1800, cacheReadTokens: 38000, cacheCreationTokens: 0, costUsd: 0.184, durationMs: 4200, ttfbMs: 600, tokensPerSecond: 78, isFastTier: true, messagesCount: 24 },
    { id: 5000, createdAt: new Date(Date.now() - 60000).toISOString(), providerName: "OpenAI Relay", userName: "bob", model: "gpt-4o", statusCode: 200, inputTokens: 18000, outputTokens: 950, cacheReadTokens: 0, cacheCreationTokens: 12000, costUsd: 0.092, durationMs: 2100, ttfbMs: 400, tokensPerSecond: 60, isFastTier: false, messagesCount: 8 },
    { id: 4999, createdAt: new Date(Date.now() - 90000).toISOString(), providerName: "Anthropic Direct", userName: "alice", model: "claude-sonnet-4", statusCode: 429, inputTokens: 0, outputTokens: 0, cacheReadTokens: 0, cacheCreationTokens: 0, costUsd: 0, durationMs: 120, errorMessage: "rate limited", messagesCount: 22 },
    { id: 4998, createdAt: new Date(Date.now() - 8000).toISOString(), providerName: "Anthropic Direct", userName: "carol", model: "claude-opus-4", statusCode: null, inputTokens: 0, outputTokens: 0, cacheReadTokens: 0, cacheCreationTokens: 0, costUsd: 0, sessionId: "sess-9", messagesCount: 3 },
  ],
  recentLogs: [],
  menuBarRunningLogs: [
    { id: 4998, createdAt: new Date(Date.now() - 8000).toISOString(), providerName: "Anthropic Direct", userName: "carol", keyName: "key-prod", model: "claude-opus-4", originalModel: "claude-opus-4", sessionId: "sess-9" },
  ],
  logSummary: { totalRequests: 1842, totalCost: 12.4567, totalTokens: 3710000, inputTokens: 1840000, outputTokens: 92000, cacheCreationTokens: 410000, cacheReadTokens: 2200000 },
  logTotal: 1842,
  providers: [
    { id: 1, name: "Anthropic Direct", providerType: "anthropic", isEnabled: true, groupTag: "prod", costMultiplier: 1, todayCalls: 920, todayCost: 6.21, limitText: "日 $50 · RPM 60", health: { circuitState: "closed" } },
    { id: 2, name: "OpenAI Relay", providerType: "openai", isEnabled: true, groupTag: "prod", costMultiplier: 1.25, todayCalls: 540, todayCost: 3.84, limitText: "总 $500", health: { circuitState: "closed" } },
    { id: 3, name: "Backup Pool", providerType: "anthropic", isEnabled: false, groupTag: "backup", costMultiplier: 2, todayCalls: 0, todayCost: 0, limitText: "无限制", health: { circuitState: "open" } },
  ],
  providerGroups: ["全部", "prod", "backup"],
  cacheStatus: {
    "5001": { state: "normal", createdTokens: 0, readTokens: 38000 },
    "5000": { state: "rebuilding", createdTokens: 12000, readTokens: 0 },
    "4998": { state: "rebuilding", createdTokens: 0, readTokens: 0 },
  },
  statusBar: { showsDetails: true, idlePrimary: "TTL $12.4", idleDetail: "1.8k req", idleCacheState: "normal", runningItems: [], hasRecentLogs: true },
  errorMessage: null,
  hasCacheAlert: true,
};
