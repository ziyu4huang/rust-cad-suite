module \top
  wire width 4 input 1 \a
  wire width 4 input 2 \b
  wire width 1 input 3 \s
  wire width 4 output 4 \y
  wire width 4 $t
  cell $and $and4
    parameter \A_SIGNED 0
    parameter \B_SIGNED 0
    parameter \A_WIDTH 4
    parameter \B_WIDTH 4
    parameter \Y_WIDTH 4
    connect \A \a
    connect \B \b
    connect \Y $t
  end
  cell $mux $sel4
    parameter \WIDTH 4
    connect \A $t
    connect \B \b
    connect \S \s
    connect \Y \y
  end
end
