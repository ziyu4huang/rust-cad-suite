# handwritten log-anchor probe (M3 continuation slice-2; D-ys5 two-run discipline)
autoidx 1
module \logprobe
  wire width 4 input 2 \a
  wire width 4 output 3 \y
  cell $and \u1
    parameter \A_SIGNED 0
    parameter \B_SIGNED 0
    parameter \A_WIDTH 4
    parameter \B_WIDTH 4
    parameter \Y_WIDTH 4
    connect \A \a
    connect \B \a
    connect \Y \y
  end
end
