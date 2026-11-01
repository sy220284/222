package com.codex.mobile;

import android.content.Context;
import android.content.SharedPreferences;
import android.security.keystore.KeyGenParameterSpec;
import android.security.keystore.KeyProperties;
import android.util.Base64;

import java.nio.charset.StandardCharsets;
import java.security.KeyStore;
import java.security.SecureRandom;

import javax.crypto.Cipher;
import javax.crypto.KeyGenerator;
import javax.crypto.SecretKey;
import javax.crypto.spec.GCMParameterSpec;

final class SecretStore {
    private static final String ALIAS = "codex-mobile-api-key-v1";
    private static final String PREFS = "codex-mobile-private";
    private static final String VALUE = "api_key_encrypted";
    private final SharedPreferences preferences;

    SecretStore(Context context) {
        preferences = context.getSharedPreferences(PREFS, Context.MODE_PRIVATE);
    }

    private SecretKey getKey() throws Exception {
        KeyStore store = KeyStore.getInstance("AndroidKeyStore");
        store.load(null);
        SecretKey saved = (SecretKey) store.getKey(ALIAS, null);
        if (saved != null) return saved;
        KeyGenerator generator = KeyGenerator.getInstance(
                KeyProperties.KEY_ALGORITHM_AES, "AndroidKeyStore");
        generator.init(new KeyGenParameterSpec.Builder(
                ALIAS, KeyProperties.PURPOSE_ENCRYPT | KeyProperties.PURPOSE_DECRYPT)
                .setBlockModes(KeyProperties.BLOCK_MODE_GCM)
                .setEncryptionPaddings(KeyProperties.ENCRYPTION_PADDING_NONE)
                .setKeySize(256).build());
        return generator.generateKey();
    }

    void save(String value) throws Exception {
        if (value.isEmpty()) {
            preferences.edit().remove(VALUE).apply();
            return;
        }
        Cipher cipher = Cipher.getInstance("AES/GCM/NoPadding");
        cipher.init(Cipher.ENCRYPT_MODE, getKey(), new SecureRandom());
        byte[] encrypted = cipher.doFinal(value.getBytes(StandardCharsets.UTF_8));
        preferences.edit().putString(VALUE,
                Base64.encodeToString(cipher.getIV(), Base64.NO_WRAP) + ":" +
                Base64.encodeToString(encrypted, Base64.NO_WRAP)).apply();
    }

    String read() throws Exception {
        String raw = preferences.getString(VALUE, "");
        if (raw == null || raw.isEmpty()) return "";
        String[] values = raw.split(":", 2);
        if (values.length != 2) throw new IllegalStateException("密钥存储格式异常");
        byte[] iv = Base64.decode(values[0], Base64.NO_WRAP);
        byte[] ciphertext = Base64.decode(values[1], Base64.NO_WRAP);
        Cipher cipher = Cipher.getInstance("AES/GCM/NoPadding");
        cipher.init(Cipher.DECRYPT_MODE, getKey(), new GCMParameterSpec(128, iv));
        return new String(cipher.doFinal(ciphertext), StandardCharsets.UTF_8);
    }
}
