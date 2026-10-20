package dev.teamy.broker.dummy;

import java.security.SecureRandom;
import java.util.Arrays;
import javax.crypto.Cipher;
import javax.crypto.spec.GCMParameterSpec;
import javax.crypto.spec.SecretKeySpec;

/** AES-256-GCM wire implementation interoperable with Rust ring. No custom cryptography. */
public final class Crypto {
    private Crypto() {}
    public static byte[] random(int count) {
        byte[] bytes = new byte[count];
        new SecureRandom().nextBytes(bytes);
        return bytes;
    }
    public static byte[] seal(byte[] key, byte[] nonce, byte[] aad, byte[] plaintext) throws Exception {
        return crypt(Cipher.ENCRYPT_MODE, key, nonce, aad, plaintext);
    }
    public static byte[] open(byte[] key, byte[] nonce, byte[] aad, byte[] ciphertext) throws Exception {
        return crypt(Cipher.DECRYPT_MODE, key, nonce, aad, ciphertext);
    }
    private static byte[] crypt(int mode, byte[] key, byte[] nonce, byte[] aad, byte[] input) throws Exception {
        if (key.length != 32 || nonce.length != 12 || input.length > 8208) throw new Exception("invalid envelope");
        Cipher cipher = Cipher.getInstance("AES/GCM/NoPadding");
        cipher.init(mode, new SecretKeySpec(key, "AES"), new GCMParameterSpec(128, nonce));
        cipher.updateAAD(aad);
        return cipher.doFinal(input);
    }
    public static String hex(byte[] bytes) {
        char[] text = new char[bytes.length * 2];
        char[] digits = "0123456789abcdef".toCharArray();
        for (int i = 0; i < bytes.length; i++) {
            text[i * 2] = digits[(bytes[i] & 255) >> 4];
            text[i * 2 + 1] = digits[bytes[i] & 15];
        }
        String result = new String(text);
        Arrays.fill(text, '\0');
        return result;
    }
    public static byte[] unhex(CharSequence text) throws Exception {
        if (text.length() > 32768 || text.length() % 2 != 0) throw new Exception("invalid encoding");
        byte[] bytes = new byte[text.length() / 2];
        try {
            for (int i = 0; i < bytes.length; i++) {
                int a = digit(text.charAt(i * 2)), b = digit(text.charAt(i * 2 + 1));
                bytes[i] = (byte)(a * 16 + b);
            }
            return bytes;
        } catch (Exception failure) {
            Arrays.fill(bytes, (byte)0);
            throw new Exception("invalid encoding");
        }
    }
    private static int digit(char c) throws Exception {
        if (c >= '0' && c <= '9') return c - '0';
        if (c >= 'a' && c <= 'f') return c - 'a' + 10;
        throw new Exception("invalid encoding");
    }
}
