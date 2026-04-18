# Imports greeter and subclasses Greeter.
require_relative 'greeter'

class AdminGreeter < Greeter
  def admin_greet(name)
    puts "Admin: #{name}"
  end
end
