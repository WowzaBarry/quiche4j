package io.quiche4j;

import java.io.IOException;
import java.net.URL;

/**
 * Helper functionality to try to resolve native library from JAR
 * in case when a system dependency is not available.
 *
 * Searches for the native library in platform-specific subdirectories first,
 * then falls back to the flat /native-libs/ directory for backwards compatibility.
 *
 * Platform directory layout:
 *   /native-libs/linux-x86_64/libquiche_jni.so
 *   /native-libs/linux-aarch64/libquiche_jni.so
 *   /native-libs/osx-aarch64/libquiche_jni.dylib
 *   /native-libs/osx-x86_64/libquiche_jni.dylib
 *   /native-libs/windows-x86_64/libquiche_jni.dll
 */
public final class NativeUtils {

    private static final String DEFAULT_DIR = "/native-libs/";

    public static void loadEmbeddedLibrary(String libname) {
        loadEmbeddedLibrary(DEFAULT_DIR, libname);
    }

    public static void loadEmbeddedLibrary(String dir, String libname) {
        // On Android, the JNI .so is expected to be packaged into the APK's
        // lib/<abi>/ directory by the Android Gradle Plugin. Java's
        // System.loadLibrary resolves it directly from there; if it didn't,
        // there is no useful JAR-extraction fallback — Android's app classpath
        // is not a conventional JAR tree and the temp-file dance below would
        // either silently no-op or crash with an opaque IOException. Fail fast
        // with a message that tells the caller where to look.
        if (isAndroid()) {
            throw new UnsatisfiedLinkError(
                "lib" + libname + " not found via System.loadLibrary on Android. "
                + "Ensure the quiche4j-jni artifact is packaged so the native "
                + "library ends up in the APK's lib/<abi>/ directory.");
        }

        final String filename = "lib" + libname;
        final String platformDir = detectPlatformDir();
        final String ext = detectExtension();

        String nativeLibraryFilepath = null;

        // Try platform-specific subdirectory first
        if (platformDir != null && ext != null) {
            final String filepath = dir + platformDir + "/" + filename + "." + ext;
            final URL url = Quiche.class.getResource(filepath);
            if (url != null) {
                nativeLibraryFilepath = filepath;
            }
        }

        // Fall back to flat directory (backwards compatibility)
        if (nativeLibraryFilepath == null) {
            String[] extensions = new String[]{"so", "dylib", "dll"};
            for (String e : extensions) {
                final String filepath = dir + filename + "." + e;
                final URL url = Quiche.class.getResource(filepath);
                if (url != null) {
                    nativeLibraryFilepath = filepath;
                    break;
                }
            }
        }

        if (nativeLibraryFilepath != null) {
            // native library found within JAR, extract and load
            try {
                final String libfile = Utils.copyFileFromJAR("libs", nativeLibraryFilepath);
                System.load(libfile);
            } catch (IOException e) {
                // no-op
            }
        }
    }

    private static boolean isAndroid() {
        // The Android runtime sets java.vm.vendor to "The Android Project" and
        // exposes java.vendor as "The Android Project" as well; java.runtime.name
        // on Android is "Android Runtime". Checking any of these works; the
        // vendor property is the traditional choice.
        return "The Android Project".equals(System.getProperty("java.vm.vendor"))
            || System.getProperty("java.vendor", "").toLowerCase().contains("android")
            || System.getProperty("java.runtime.name", "").toLowerCase().contains("android");
    }

    private static String detectPlatformDir() {
        String os = System.getProperty("os.name", "").toLowerCase();
        String arch = System.getProperty("os.arch", "").toLowerCase();

        String osName;
        if (os.contains("linux")) {
            osName = "linux";
        } else if (os.contains("mac") || os.contains("darwin")) {
            osName = "osx";
        } else if (os.contains("win")) {
            osName = "windows";
        } else {
            return null;
        }

        String archName;
        if (arch.equals("amd64") || arch.equals("x86_64")) {
            archName = "x86_64";
        } else if (arch.equals("aarch64") || arch.equals("arm64")) {
            archName = "aarch64";
        } else {
            return null;
        }

        return osName + "-" + archName;
    }

    private static String detectExtension() {
        String os = System.getProperty("os.name", "").toLowerCase();
        if (os.contains("linux")) return "so";
        if (os.contains("mac") || os.contains("darwin")) return "dylib";
        if (os.contains("win")) return "dll";
        return null;
    }
}
