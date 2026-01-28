import { describe, expect, it, vi, beforeEach } from 'vitest';
import { api } from '../api';

describe('api', () => {
  const mockFetch = vi.fn();
  vi.stubGlobal('fetch', mockFetch);

  beforeEach(() => {
    mockFetch.mockClear();
  });

  // Helper to create a proper mock response
  function createFetchResponse(data: any, ok = true, status = 200, contentType = 'application/json') {
    return {
      ok,
      status,
      headers: {
        get: (name: string) => (name.toLowerCase() === 'content-type' ? contentType : null),
      },
      json: async () => data,
      text: async () => (typeof data === 'string' ? data : JSON.stringify(data)),
    } as Response;
  }

  it('listProfiles calls correct endpoint', async () => {
    mockFetch.mockResolvedValueOnce(createFetchResponse([{ name: 'default' }]));

    const result = await api.listProfiles();
    expect(mockFetch).toHaveBeenCalledWith(expect.stringContaining('/admin/api/profiles'), expect.anything());
    expect(result).toEqual([{ name: 'default' }]);
  });

  it('saveAppSettings handles partial updates', async () => {
    mockFetch.mockResolvedValueOnce(createFetchResponse(null, true, 204, ''));

    await api.saveAppSettings({ language: 'en-US' });
    expect(mockFetch).toHaveBeenCalledWith(
      expect.stringContaining('/admin/api/settings'),
      expect.objectContaining({
        method: 'POST',
        body: JSON.stringify({ language: 'en-US' }),
      })
    );
  });

  it('handles API errors with messages', async () => {
    mockFetch.mockResolvedValueOnce(createFetchResponse({ error: 'invalid format' }, false, 400));

    await expect(api.getDnsConfig()).rejects.toThrow('invalid format');
  });

  it('handles network failures', async () => {
    mockFetch.mockRejectedValueOnce(new Error('Failed to fetch'));
    await expect(api.listProfiles()).rejects.toThrow('Failed to fetch');
  });

  it('importProfile sends correct payload', async () => {
    mockFetch.mockResolvedValueOnce(createFetchResponse({ profile: { name: 'new' }, rebuild_scheduled: false }));

    await api.importProfile('name1', 'http://url', true);
    expect(mockFetch).toHaveBeenCalledWith(
      expect.stringContaining('/admin/api/profiles/import'),
      expect.objectContaining({
        method: 'POST',
        body: JSON.stringify({ name: 'name1', url: 'http://url', activate: true }),
      })
    );
  });

  it('deleteProfile sends DELETE request', async () => {
    mockFetch.mockResolvedValueOnce(createFetchResponse(null, true, 204, ''));
    await api.deleteProfile('p1');
    expect(mockFetch).toHaveBeenCalledWith(
      expect.stringContaining('/admin/api/profiles/p1'),
      expect.objectContaining({
        method: 'DELETE',
      })
    );
  });

  it('flushFakeIpCache returns removal status', async () => {
    mockFetch.mockResolvedValueOnce(createFetchResponse({ removed: true }));
    const result = await api.flushFakeIpCache();
    expect(result.removed).toBe(true);
    expect(mockFetch).toHaveBeenCalledWith(
      expect.stringContaining('/admin/api/fake-ip/flush'),
      expect.objectContaining({ method: 'POST' })
    );
  });

  it('clearProfileSubscription sends DELETE request', async () => {
    mockFetch.mockResolvedValueOnce(createFetchResponse({ name: 'p1' }));
    await api.clearProfileSubscription('p1');
    expect(mockFetch).toHaveBeenCalledWith(
      expect.stringContaining('/admin/api/profiles/p1/subscription'),
      expect.objectContaining({ method: 'DELETE' })
    );
  });

  it('clearProfiles sends POST', async () => {
    mockFetch.mockResolvedValueOnce(createFetchResponse({ profile: { name: 'default' }, rebuild_scheduled: true }));
    await api.clearProfiles();
    expect(mockFetch).toHaveBeenCalledWith(expect.stringContaining('/admin/api/profiles/clear'), expect.objectContaining({ method: 'POST' }));
  });

  it('openProfile sends POST', async () => {
    mockFetch.mockResolvedValueOnce(createFetchResponse(null, true, 204));
    await api.openProfile('p1');
    expect(mockFetch).toHaveBeenCalledWith(expect.stringContaining('/admin/api/profiles/open'), expect.objectContaining({ method: 'POST', body: JSON.stringify({ name: 'p1' }) }));
  });

  it('editor functions work', async () => {
    mockFetch.mockResolvedValueOnce(createFetchResponse({ editor: 'code' }));
    const res = await api.getEditor();
    expect(res.editor).toBe('code');

    mockFetch.mockResolvedValueOnce(createFetchResponse(null, true, 204));
    await api.setEditor('vim');
    expect(mockFetch).toHaveBeenCalledWith(expect.stringContaining('/admin/api/editor'), expect.objectContaining({ method: 'POST', body: JSON.stringify({ editor: 'vim' }) }));

    mockFetch.mockResolvedValueOnce(createFetchResponse({ editor: 'picked' }));
    const picked = await api.pickEditor();
    expect(picked.editor).toBe('picked');
  });

  it('core version functions work', async () => {
    mockFetch.mockResolvedValueOnce(createFetchResponse({ current: 'v1', versions: ['v1'] }));
    await api.listCoreVersions();
    
    mockFetch.mockResolvedValueOnce(createFetchResponse(null, true, 204));
    await api.activateCoreVersion('v1');
    expect(mockFetch).toHaveBeenCalledWith(expect.stringContaining('/admin/api/core/activate'), expect.objectContaining({ method: 'POST', body: JSON.stringify({ version: 'v1' }) }));
  });

  it('config getter/setter work', async () => {
    const cfgs = [
        { name: 'FakeIp', get: api.getFakeIpConfig, save: api.saveFakeIpConfig, url: '/admin/api/fake-ip' },
        { name: 'RuleProviders', get: api.getRuleProviders, save: api.saveRuleProviders, url: '/admin/api/rule-providers' },
        { name: 'Rules', get: api.getRules, save: api.saveRules, url: '/admin/api/rules' },
        { name: 'Tun', get: api.getTunConfig, save: api.saveTunConfig, url: '/admin/api/tun' }
    ];

    for (const c of cfgs) {
        mockFetch.mockResolvedValueOnce(createFetchResponse({}));
        await (api as any)[c.get.name]();
        expect(mockFetch).toHaveBeenCalledWith(expect.stringContaining(c.url), expect.anything());

        mockFetch.mockClear();
        mockFetch.mockResolvedValueOnce(createFetchResponse({}));
        await (api as any)[c.save.name]({});
        expect(mockFetch).toHaveBeenCalledWith(expect.stringContaining(c.url), expect.objectContaining({ method: 'POST' }));
    }
  });

  it('syncWebDavNow and testWebDav work', async () => {
    mockFetch.mockResolvedValueOnce(createFetchResponse({}));
    await api.syncWebDavNow();
    expect(mockFetch).toHaveBeenCalledWith(expect.stringContaining('/admin/api/webdav/sync'), expect.objectContaining({ method: 'POST' }));

    mockFetch.mockResolvedValueOnce(createFetchResponse(null, true, 204));
    await api.testWebDav({} as any);
    expect(mockFetch).toHaveBeenCalledWith(expect.stringContaining('/admin/api/webdav/test'), expect.objectContaining({ method: 'POST' }));
  });

  it('getRebuildStatus works', async () => {
    mockFetch.mockResolvedValueOnce(createFetchResponse({ rebuilding: false }));
    await api.getRebuildStatus();
    expect(mockFetch).toHaveBeenCalledWith(expect.stringContaining('/admin/api/rebuild/status'), expect.anything());
  });

  it('handles timeout correctly', async () => {
    const abortError = new Error('The user aborted a request.');
    abortError.name = 'AbortError';
    mockFetch.mockRejectedValueOnce(abortError);
    
    await expect(api.listProfiles()).rejects.toThrow('超时');
  });

  it('handles non-json error responses', async () => {
    mockFetch.mockResolvedValueOnce(createFetchResponse('Raw error message', false, 500, 'text/plain'));
    await expect(api.getAppSettings()).rejects.toThrow('Raw error message');
  });

  it('handles empty success responses', async () => {
    mockFetch.mockResolvedValueOnce(createFetchResponse(null, true, 204, ''));
    // setEditor is a real method
    await api.setEditor('vim');
    
    // Test that request logic works correctly when response is ok but no content
    mockFetch.mockResolvedValueOnce(createFetchResponse(null, true, 204, ''));
    const profiles = await api.listProfiles();
    expect(profiles).toBeNull();
  });

  it('handles non-JSON error response', async () => {
    mockFetch.mockResolvedValue({
      ok: false,
      status: 403,
      text: () => Promise.resolve('Forbidden Text'),
      json: () => Promise.reject(new Error('Invalid JSON')),
      headers: { get: () => 'text/html' }
    } as any);

    await expect(api.listProfiles()).rejects.toThrow('Forbidden Text');
  });

  it('handles empty error response text', async () => {
    vi.mocked(fetch).mockResolvedValue({
      ok: false,
      status: 404,
      statusText: 'Not Found',
      text: () => Promise.resolve(''),
      json: () => Promise.reject(new Error()),
      headers: { get: () => 'text/plain' }
    } as any);

    await expect(api.listProfiles()).rejects.toThrow('Not Found');
  });
});
