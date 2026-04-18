# Requires greeter and calls Greeter#greet.
require_relative 'greeter'

def run
  g = Greeter.new
  g.greet('world')
end
