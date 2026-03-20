import Foundation

/// A protocol defining repository operations.
protocol Repository {
    associatedtype Entity
    func findById(_ id: String) -> Entity?
    func findAll() -> [Entity]
    func save(_ entity: Entity) -> Entity
    func delete(_ id: String)
}

/// A simple struct.
struct Point {
    let x: Double
    let y: Double

    func distance(to other: Point) -> Double {
        let dx = x - other.x
        let dy = y - other.y
        return (dx * dx + dy * dy).squareRoot()
    }
}

/// A class with inheritance.
class Animal {
    let name: String

    init(name: String) {
        self.name = name
    }

    func speak() -> String {
        return ""
    }
}

/// Subclass.
class Dog: Animal {
    override func speak() -> String {
        return "Woof!"
    }
}

/// An enum with associated values.
enum Result<Success, Failure> {
    case success(Success)
    case failure(Failure)

    var isSuccess: Bool {
        switch self {
        case .success: return true
        case .failure: return false
        }
    }
}

/// A simple enum.
enum Direction {
    case north, south, east, west

    var isHorizontal: Bool {
        return self == .east || self == .west
    }
}

/// An enum with raw values.
enum HTTPStatusCode: Int {
    case ok = 200
    case notFound = 404
    case serverError = 500
}

/// Extensions.
extension String {
    func isValidEmail() -> Bool {
        return contains("@") && contains(".")
    }

    var isBlank: Bool {
        return trimmingCharacters(in: .whitespaces).isEmpty
    }
}

extension Array where Element: Comparable {
    func secondSmallest() -> Element? {
        guard count >= 2 else { return nil }
        return sorted()[1]
    }
}

/// Protocol with default implementation via extension.
protocol Drawable {
    func draw()
}

extension Drawable {
    func draw() {
        print("Default drawing")
    }
}

/// A generic class.
class NetworkManager<T: Codable> {
    let baseURL: URL

    init(baseURL: URL) {
        self.baseURL = baseURL
    }

    func fetch(endpoint: String) -> T? {
        return nil
    }
}

/// Actor (Swift 5.5+).
actor BankAccount {
    private var balance: Double

    init(balance: Double) {
        self.balance = balance
    }

    func deposit(_ amount: Double) {
        balance += amount
    }

    func withdraw(_ amount: Double) -> Bool {
        guard balance >= amount else { return false }
        balance -= amount
        return true
    }
}

/// Top-level function.
func retry<T>(times: Int, block: () throws -> T) rethrows -> T {
    for _ in 0..<(times - 1) {
        do { return try block() } catch { continue }
    }
    return try block()
}

/// A typealias.
typealias Completion = (Result<String, Error>) -> Void

/// Property wrapper.
@propertyWrapper
struct Clamped<Value: Comparable> {
    var wrappedValue: Value {
        didSet { wrappedValue = min(max(wrappedValue, range.lowerBound), range.upperBound) }
    }
    let range: ClosedRange<Value>

    init(wrappedValue: Value, _ range: ClosedRange<Value>) {
        self.range = range
        self.wrappedValue = min(max(wrappedValue, range.lowerBound), range.upperBound)
    }
}
