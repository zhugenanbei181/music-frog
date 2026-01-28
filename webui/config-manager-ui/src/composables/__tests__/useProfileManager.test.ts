import { describe, expect, it, vi, beforeEach } from 'vitest';
import { ref, nextTick } from 'vue';
import { useProfileManager } from '../useProfileManager';
import { api } from '../../api';

vi.mock('../../api', () => ({
  api: {
    listProfiles: vi.fn(),
    getProfile: vi.fn(),
    switchProfile: vi.fn(),
    clearProfiles: vi.fn(),
    importProfile: vi.fn(),
    saveProfile: vi.fn(),
    deleteProfile: vi.fn(),
    setProfileSubscription: vi.fn(),
    updateProfileNow: vi.fn(),
    openProfile: vi.fn(),
  },
}));

vi.mock('vue-i18n', () => ({
  useI18n: () => ({
    t: (key: string, _params?: any) => key,
  }),
}));

function buildOptions() {
  return {
    busy: {
      busy: ref(false),
      startBusy: vi.fn(),
      updateBusyDetail: vi.fn(),
      endBusy: vi.fn(),
    },
    setStatus: vi.fn(),
    pushToast: vi.fn(),
    waitForRebuild: vi.fn().mockResolvedValue(undefined),
    scrollToEditor: vi.fn(),
  };
}

describe('useProfileManager', () => {
  beforeEach(() => {
    vi.clearAllMocks();
  });

  it('marks editor as dirty when modified', async () => {
    const options = buildOptions();
    const { editor, editorDirty } = useProfileManager(options);

    expect(editorDirty.value).toBe(false);
    
    editor.content = 'new content';
    await nextTick();
    
    expect(editorDirty.value).toBe(true);
  });

  it('saves profile and resets dirty flag', async () => {
    const options = buildOptions();
    vi.mocked(api.saveProfile).mockResolvedValue({
      profile: { name: 'p1' } as any,
      rebuild_scheduled: false,
    });
    vi.mocked(api.listProfiles).mockResolvedValue([]);

    const { editor, editorDirty, saveProfile } = useProfileManager(options);
    
    editor.name = 'p1';
    editor.content = 'c1';
    editorDirty.value = true;

    await saveProfile();

    expect(api.saveProfile).toHaveBeenCalledWith('p1', 'c1', false);
    expect(editorDirty.value).toBe(false);
  });

  it('handles delete profile errors', async () => {
    const options = buildOptions();
    const promptMock = vi.fn().mockReturnValue('p1');
    vi.stubGlobal('window', { prompt: promptMock });
    vi.mocked(api.deleteProfile).mockRejectedValue(new Error('Delete Fail'));

    const { deleteProfile } = useProfileManager(options);
    await deleteProfile('p1');

    expect(options.pushToast).toHaveBeenCalledWith('Delete Fail', 'error');
    vi.unstubAllGlobals();
  });

  it('imports profile from URL', async () => {
    const options = buildOptions();
    vi.mocked(api.importProfile).mockResolvedValue({
      profile: { name: 'new-sub' } as any,
      rebuild_scheduled: true,
    });
    vi.mocked(api.listProfiles).mockResolvedValue([]);

    const { importForm, importProfile } = useProfileManager(options);
    
    importForm.name = 'new-sub';
    importForm.url = 'http://sub.link';
    importForm.activate = true;

    await importProfile();

    expect(api.importProfile).toHaveBeenCalledWith('new-sub', 'http://sub.link', true);
    expect(options.waitForRebuild).toHaveBeenCalled();
  });

  it('updates profile subscription', async () => {
    const options = buildOptions();
    const { updateSubscription } = useProfileManager(options);
    vi.mocked(api.setProfileSubscription).mockResolvedValue({} as any);
    vi.mocked(api.listProfiles).mockResolvedValue([]);

    await updateSubscription({ name: 'p1', url: 'http://u', auto_update_enabled: true });
    expect(api.setProfileSubscription).toHaveBeenCalledWith('p1', expect.objectContaining({
        url: 'http://u',
        auto_update_enabled: true
    }));
    expect(options.setStatus).toHaveBeenCalledWith('app.save_sub_success', 'p1');
  });

  it('updates profile subscription immediately', async () => {
    const options = buildOptions();
    const { updateSubscriptionNow } = useProfileManager(options);
    vi.mocked(api.updateProfileNow).mockResolvedValue({
        profile: { name: 'p1' } as any,
        rebuild_scheduled: false
    });
    vi.mocked(api.listProfiles).mockResolvedValue([]);

    await updateSubscriptionNow('p1');
    expect(api.updateProfileNow).toHaveBeenCalledWith('p1');
    expect(options.setStatus).toHaveBeenCalledWith('app.update_sub_success', 'p1');
  });

  it('imports local files', async () => {
    const options = buildOptions();
    const { importLocal, onLocalFileChange } = useProfileManager(options);
    vi.mocked(api.saveProfile).mockResolvedValue({
        profile: { name: 'local' } as any,
        rebuild_scheduled: false
    });
    vi.mocked(api.listProfiles).mockResolvedValue([]);

    const file = new File(['content'], 'test.yaml', { type: 'text/yaml' });
    onLocalFileChange(file);
    
    // File.text() mock
    const originalText = File.prototype.text;
    File.prototype.text = vi.fn().mockResolvedValue('content');

    await importLocal();
    expect(api.saveProfile).toHaveBeenCalledWith('test', 'content', false);
    
    File.prototype.text = originalText;
  });
});
