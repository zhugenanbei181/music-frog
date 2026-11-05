import java.io.File;
import java.io.IOException;
import java.nio.file.Files;
import java.nio.file.StandardCopyOption;
import java.util.ArrayList;
import java.util.LinkedHashMap;
import java.util.Map;
import org.gradle.api.DefaultTask;
import org.gradle.api.file.DirectoryProperty;
import org.gradle.api.file.DirectoryProperty;
import org.gradle.api.provider.ListProperty;
import org.gradle.api.tasks.Input;
import org.gradle.api.tasks.OutputDirectory;
import org.gradle.api.tasks.TaskAction;

public abstract class GenerateSharedFonts extends DefaultTask {
    @org.gradle.api.tasks.InputDirectory
    @org.gradle.api.tasks.PathSensitive(org.gradle.api.tasks.PathSensitivity.NONE)
    public abstract DirectoryProperty getSharedFontDirectory();

    @Input
    public abstract ListProperty<String> getFontNames();

    @OutputDirectory
    public abstract DirectoryProperty getOutputDirectory();

    @TaskAction
    public void generate() {
        File sharedFonts = getSharedFontDirectory().get().getAsFile();
        File target = new File(getOutputDirectory().get().getAsFile(), "font");
        if (!target.isDirectory() && !target.mkdirs()) {
            throw new IllegalStateException("Cannot create shared font directory: " + target);
        }
        Map<String, String> faces = new LinkedHashMap<>();
        faces.put("inter_regular.ttf", "Inter-Regular.ttf");
        faces.put("inter_medium.ttf", "Inter-Medium.ttf");
        faces.put("inter_semibold.ttf", "Inter-SemiBold.ttf");
        faces.put("jetbrains_mono_regular.ttf", "JetBrainsMono-Regular.ttf");
        faces.put("cjk_regular.ttf", "CJK-Regular.ttf");
        if (!getFontNames().get().equals(new ArrayList<>(faces.keySet()))) {
            throw new IllegalStateException("Shared font resource names drifted from the contract authority");
        }
        for (Map.Entry<String, String> face : faces.entrySet()) {
            File asset = new File(sharedFonts, face.getValue());
            if (!asset.isFile()) {
                throw new IllegalStateException("Shared font face is missing: " + asset);
            }
            try {
                Files.copy(
                    asset.toPath(),
                    new File(target, face.getKey()).toPath(),
                    StandardCopyOption.REPLACE_EXISTING
                );
            } catch (IOException error) {
                throw new IllegalStateException("Cannot copy shared font face: " + asset, error);
            }
        }
    }
}
