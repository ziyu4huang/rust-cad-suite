# handwritten purge-semantics probe (M3 continuation; D-ys5 two-run discipline)
autoidx 1

# min2a: public wire with driver but no reader (purge diverges) + used public wire
module \min2a
  wire width 4 input 2 \a
  wire width 4 output 3 \y
  wire width 4 \drv_no_read
  wire width 4 \used
  cell $and \u1
    parameter \A_SIGNED 0
    parameter \B_SIGNED 0
    parameter \A_WIDTH 4
    parameter \B_WIDTH 4
    parameter \Y_WIDTH 4
    connect \A \a
    connect \B \a
    connect \Y \drv_no_read
  end
  cell $and \u2
    parameter \A_SIGNED 0
    parameter \B_SIGNED 0
    parameter \A_WIDTH 4
    parameter \B_WIDTH 4
    parameter \Y_WIDTH 4
    connect \A \a
    connect \B \a
    connect \Y \used
  end
  connect \y \used
end

# min2b: public wire with no driver and no reader (removed in both modes)
module \min2b
  wire width 4 input 2 \a
  wire width 4 output 3 \y
  wire width 4 \orphan
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

# min2c: private wire with driver but no reader (removed in both modes)
module \min2c
  wire width 4 input 2 \a
  wire width 4 output 3 \y
  wire width 4 $priv_no_read
  cell $and \u1
    parameter \A_SIGNED 0
    parameter \B_SIGNED 0
    parameter \A_WIDTH 4
    parameter \B_WIDTH 4
    parameter \Y_WIDTH 4
    connect \A \a
    connect \B \a
    connect \Y $priv_no_read
  end
  cell $and \u2
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
