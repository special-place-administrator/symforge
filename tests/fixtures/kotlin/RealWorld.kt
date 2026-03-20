package com.example.test

/**
 * A sealed interface representing a result.
 */
sealed interface Result<out T> {
    fun isSuccess(): Boolean
}

/**
 * Represents a successful result.
 */
data class Success<T>(val value: T) : Result<T> {
    override fun isSuccess(): Boolean = true
}

/**
 * Represents a failed result.
 */
data class Failure(val error: Throwable) : Result<Nothing> {
    override fun isSuccess(): Boolean = false
}

/**
 * Direction enum class.
 */
enum class Direction {
    NORTH, SOUTH, EAST, WEST;

    fun isHorizontal(): Boolean = this == EAST || this == WEST
}

/**
 * Color enum class with properties.
 */
enum class Color(val rgb: Int) {
    RED(0xFF0000),
    GREEN(0x00FF00),
    BLUE(0x0000FF);

    fun containsRed(): Boolean = (rgb and 0xFF0000) != 0
}

/**
 * A regular interface.
 */
interface Repository<T> {
    suspend fun findById(id: String): T?
    suspend fun findAll(): List<T>
    suspend fun save(entity: T): T
    suspend fun delete(id: String)
}

/**
 * Abstract class.
 */
abstract class BaseViewModel {
    abstract fun onStart()
    open fun onStop() {}
}

/**
 * Companion object and nested class.
 */
class NetworkClient private constructor(val baseUrl: String) {

    companion object {
        fun create(baseUrl: String): NetworkClient = NetworkClient(baseUrl)
    }

    class Builder {
        private var baseUrl: String = ""

        fun baseUrl(url: String): Builder {
            baseUrl = url
            return this
        }

        fun build(): NetworkClient = NetworkClient(baseUrl)
    }
}

/**
 * Object declaration (singleton).
 */
object Logger {
    fun log(message: String) {
        println(message)
    }
}

/**
 * Top-level functions.
 */
fun <T> retry(times: Int, block: () -> T): T {
    repeat(times - 1) {
        try {
            return block()
        } catch (e: Exception) {
            // retry
        }
    }
    return block()
}

suspend fun fetchData(url: String): String {
    return "data from $url"
}

/**
 * Extension functions.
 */
fun String.isValidEmail(): Boolean {
    return this.contains("@") && this.contains(".")
}

fun List<Int>.secondOrNull(): Int? = if (size >= 2) this[1] else null

/**
 * Type alias.
 */
typealias StringMap = Map<String, String>

/**
 * Annotation class.
 */
annotation class Inject

/**
 * Inline class / value class.
 */
@JvmInline
value class Email(val value: String)
