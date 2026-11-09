package org.wanten.remoteinput

import java.net.InetSocketAddress
import java.security.MessageDigest
import java.security.SecureRandom
import java.security.cert.Certificate
import java.security.cert.CertificateException
import java.security.cert.X509Certificate
import javax.net.ssl.SSLContext
import javax.net.ssl.SSLPeerUnverifiedException
import javax.net.ssl.SSLSocket
import javax.net.ssl.TrustManager
import javax.net.ssl.X509TrustManager

fun sha256Hex(cert: Certificate): String {
    val digest = MessageDigest.getInstance("SHA-256").digest(cert.encoded)
    return digest.joinToString("") { "%02X".format(it) }
}

fun trustAllManager(): X509TrustManager {
    return object : X509TrustManager {
        override fun checkClientTrusted(chain: Array<X509Certificate>, authType: String) {}
        override fun checkServerTrusted(chain: Array<X509Certificate>, authType: String) {}
        override fun getAcceptedIssuers(): Array<X509Certificate> = arrayOf()
    }
}

fun fetchServerFingerprint(host: String, port: Int, timeoutMs: Int = 8000): String {
    val ctx = SSLContext.getInstance("TLS")
    ctx.init(null, arrayOf<TrustManager>(trustAllManager()), SecureRandom())
    val socket = ctx.socketFactory.createSocket() as SSLSocket
    socket.connect(InetSocketAddress(host, port), timeoutMs)
    socket.soTimeout = timeoutMs
    try {
        socket.startHandshake()
        val chain = socket.session.peerCertificates
        if (chain.isEmpty()) throw SSLPeerUnverifiedException("empty chain")
        return sha256Hex(chain[0])
    } finally {
        socket.close()
    }
}

fun pinTrustManager(expectedHex: String): X509TrustManager {
    return object : X509TrustManager {
        override fun checkClientTrusted(chain: Array<X509Certificate>, authType: String) {}
        override fun checkServerTrusted(chain: Array<X509Certificate>, authType: String) {
            if (chain.isEmpty()) throw CertificateException("empty chain")
            if (!sha256Hex(chain[0]).equals(expectedHex, ignoreCase = true)) {
                throw CertificateException("pin mismatch")
            }
        }
        override fun getAcceptedIssuers(): Array<X509Certificate> = arrayOf()
    }
}
