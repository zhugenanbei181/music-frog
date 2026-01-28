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
    t: (key: string, params?: any) => key,
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
      profile: { name: 'p1' },
      rebuild_scheduled: false,
    } as any);
    vi.mocked(api.listProfiles).mockResolvedValue([]);

    const { editor, editorDirty, saveProfile } = useProfileManager(options);
    
    editor.name = 'p1';
    editor.content = 'c1';
    editorDirty.value = true;

    await saveProfile();

    expect(api.saveProfile).toHaveBeenCalledWith('p1', 'c1', false);
    expect(editorDirty.value).toBe(false);
    expect(options.busy.startBusy).toHaveBeenCalled();
  });

  it('handles delete with confirmation', async () => {
    const options = buildOptions();
    // Mock window.prompt
    const promptMock = vi.fn().mockReturnValue('delete-me');
    vi.stubGlobal('window', { prompt: promptMock });
    vi.mocked(api.listProfiles).mockResolvedValue([]);

    const { deleteProfile } = useProfileManager(options);
    
    await deleteProfile('delete-me');

    expect(promptMock).toHaveBeenCalled();
    expect(api.deleteProfile).toHaveBeenCalledWith('delete-me');
    expect(options.setStatus).toHaveBeenCalledWith('app.delete_success', 'delete-me');
    
    vi.unstubAllGlobals();
  });

  it('aborts delete if confirmation mismatch', async () => {
    const options = buildOptions();
    const promptMock = vi.fn().mockReturnValue('wrong-name');
    vi.stubGlobal('window', { prompt: promptMock });

    const { deleteProfile } = useProfileManager(options);
    await deleteProfile('p1');

    expect(api.deleteProfile).not.toHaveBeenCalled();
    vi.unstubAllGlobals();
  });

  it('imports profile from URL', async () => {
    const options = buildOptions();
    vi.mocked(api.importProfile).mockResolvedValue({
      profile: { name: 'new-sub' },
      rebuild_scheduled: true,
    } as any);
    vi.mocked(api.listProfiles).mockResolvedValue([]);

    const { importForm, importProfile } = useProfileManager(options);
    
    importForm.name = 'new-sub';
    importForm.url = 'http://sub.link';
    importForm.activate = true;

    await importProfile();

    expect(api.importProfile).toHaveBeenCalledWith('new-sub', 'http://sub.link', true);
    expect(options.waitForRebuild).toHaveBeenCalled();
    expect(importForm.url).toBe(''); // Form cleared
  });

  it('loads profile detail into editor', async () => {
    const options = buildOptions();
    vi.mocked(api.getProfile).mockResolvedValue({
      name: 'p1',
      content: 'yaml content',
      active: true,
    } as any);

    const { editor, loadProfile, editorDirty } = useProfileManager(options);
    
    await loadProfile('p1');

    expect(editor.name).toBe('p1');
        expect(editor.content).toBe('yaml content');
        expect(editorDirty.value).toBe(false); // Should not be dirty immediately after load
        expect(options.scrollToEditor).toHaveBeenCalled();
      });
    
        it('imports local file correctly', async () => {
          const options = buildOptions();
          vi.mocked(api.saveProfile).mockResolvedValue({
            profile: { name: 'local-file' },
            rebuild_scheduled: true,
          } as any);
          vi.mocked(api.listProfiles).mockResolvedValue([]);
      
          // Manually mock File.text because jsdom might not implement it fully in all versions
          const originalText = File.prototype.text;
          File.prototype.text = vi.fn().mockResolvedValue('mock content');
      
          const { localForm, importLocal, onLocalFileChange } = useProfileManager(options);
          
          // Simulate file selection
          const mockFile = new File([''], 'test.yaml', { type: 'text/yaml' });
          onLocalFileChange(mockFile);
          expect(localForm.name).toBe('test');
      
          await importLocal();
      
          expect(api.saveProfile).toHaveBeenCalledWith('test', 'mock content', false);
          expect(options.waitForRebuild).toHaveBeenCalled();
          expect(localForm.file).toBeNull();
      
          File.prototype.text = originalText;
        });
            it('switches profile and waits for rebuild', async () => {
        const options = buildOptions();
        vi.mocked(api.switchProfile).mockResolvedValue({
          profile: { name: 'target' },
          rebuild_scheduled: true,
        } as any);
        vi.mocked(api.listProfiles).mockResolvedValue([]);
    
        const { switchProfile } = useProfileManager(options);
        
        await switchProfile('target');
    
        expect(api.switchProfile).toHaveBeenCalledWith('target');
        expect(options.busy.startBusy).toHaveBeenCalled();
        expect(options.waitForRebuild).toHaveBeenCalled();
        expect(options.setStatus).toHaveBeenCalledWith(expect.any(String), 'target');
      });
    });
    