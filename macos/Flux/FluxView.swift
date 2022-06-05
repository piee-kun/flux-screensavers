//
//  FluxView.swift
//  Flux
//
//  Created by Sander Melnikov on 05/02/2022.
//
import ScreenSaver
import Cocoa
import OpenGL.GL3

let SETTINGS = """
{
    "mode": "Normal",
    "viscosity": 5.0,
    "velocityDissipation": 0.0,
    "startingPressure": "Inherit",
    "fluidSize": 128,
    "fluidSimulationFrameRate": 60.0,
    "diffusionIterations": 5,
    "pressureIterations": 20,
    "colorScheme": "Peacock",
    "lineLength": 300.0,
    "lineWidth": 5.0,
    "lineBeginOffset": 0.5,
    "lineVariance": 0.5,
    "viewScale": 1.6,
    "gridSpacing": 21,
    "noiseChannels": [
        {
            "scale": 2.5,
            "multiplier": 1.0,
            "offsetIncrement": 0.0015
        },
        {
            "scale": 15.0,
            "multiplier": 0.7,
            "offsetIncrement": 0.0015
        },
        {
            "scale": 30.0,
            "multiplier": 0.5,
            "offsetIncrement": 0.0015
        }
    ]
}
"""

class FluxView: ScreenSaverView {
    var pixelFormat: NSOpenGLPixelFormat!
    var openGLContext: NSOpenGLContext!
    var displayLink: CVDisplayLink!
    var flux: OpaquePointer?
    var currentTime: Double = 0

    override init?(frame: NSRect, isPreview: Bool) {
        super.init(frame: frame, isPreview: isPreview)

        let attributes: [NSOpenGLPixelFormatAttribute] = [
            NSOpenGLPixelFormatAttribute(NSOpenGLPFAAccelerated),
            NSOpenGLPixelFormatAttribute(NSOpenGLPFATripleBuffer),
            NSOpenGLPixelFormatAttribute(NSOpenGLPFAColorSize), 32,
            NSOpenGLPixelFormatAttribute(NSOpenGLPFAOpenGLProfile),
            NSOpenGLPixelFormatAttribute(NSOpenGLProfileVersion4_1Core),
            0
          ]
        guard let pixelFormat = NSOpenGLPixelFormat(attributes: attributes) else {
            print("Cannot construct OpenGL pixel format.")
            return nil
        }
        self.pixelFormat = pixelFormat
        guard let context = NSOpenGLContext(format: pixelFormat, share: nil) else {
            print("Cannot create OpenGL context.")
            return nil
        }
        context.setValues([1], for: .swapInterval)
        openGLContext = context
        
        displayLink = makeDisplayLink()
    }
    
    // Debug in app
    required init?(coder decoder: NSCoder) {
        super.init(coder: decoder)
    }
    
    // This is helpful if you need access to window
//    override func viewDidMoveToSuperview() {
//        super.viewDidMoveToSuperview()
//        if let window = superview?.window {
//            displayLink = makeDisplayLink()
//        }
//    }
    
    private func makeDisplayLink() -> CVDisplayLink? {
        func displayLinkOutputCallback(_ displayLink: CVDisplayLink, _ nowPtr: UnsafePointer<CVTimeStamp>, _ outputTimePtr: UnsafePointer<CVTimeStamp>, _ flagsIn: CVOptionFlags, _ flagsOut: UnsafeMutablePointer<CVOptionFlags>, _ displayLinkContext: UnsafeMutableRawPointer?) -> CVReturn {
            
            let _self = unsafeBitCast(displayLinkContext, to: FluxView.self)
            let outputTime = outputTimePtr.pointee
             _self.currentTime = 1000.0 * Double(outputTime.videoTime) / Double(outputTime.videoTimeScale)
            
            // Show FPS
            // let fps = (outputTime.rateScalar * Double(outputTime.videoTimeScale) / Double(outputTime.videoRefreshPeriod))
            //  print("FPS:\t \(fps)")

            _self.animateOneFrame()

            return kCVReturnSuccess
        }
        
        var link: CVDisplayLink?
        CVDisplayLinkCreateWithActiveCGDisplays(&link)
        CVDisplayLinkSetOutputCallback(link!, displayLinkOutputCallback, UnsafeMutableRawPointer(Unmanaged.passUnretained(self).toOpaque()))
        CVDisplayLinkSetCurrentCGDisplayFromOpenGLContext(link!, openGLContext!.cglContextObj!, pixelFormat!.cglPixelFormatObj!)
        
        return link
    }
    
    override func lockFocus() {
      super.lockFocus()
      if openGLContext!.view != self {
        openGLContext!.view = self
      }
    }
    
    override class func backingStoreType() -> NSWindow.BackingStoreType {
        return NSWindow.BackingStoreType.buffered
    }
    
    override func startAnimation() {
        // Don’t call super because we’re managing our own timer.
        
        lockFocus()
        openGLContext?.lock()
        openGLContext?.makeCurrentContext()
        
        let logical_size = frame.size
        let physical_size = window!.convertToBacking(frame).size;
        print(logical_size, physical_size)
        guard let flux = flux_new(
            Float(logical_size.width),
            Float(logical_size.height),
            Float(physical_size.width),
            Float(physical_size.height),
            SETTINGS
        ) else {
            // TODO: question the FFI for the last error
            print("Can’t initialize Flux")
            openGLContext?.unlock()
            return
        }
        
        self.flux = flux
        openGLContext?.unlock()
        
        CVDisplayLinkStart(displayLink!)
    }
    
    override func stopAnimation() {
        // Don’t call super. See startAnimation.
        CVDisplayLinkStop(displayLink!)
        flux_destroy(flux!)
    }

    private func drawView() -> CVReturn {
        openGLContext.lock()
        openGLContext.makeCurrentContext()

        flux_animate(flux, currentTime)

        openGLContext.flushBuffer()
        openGLContext.unlock()

        return kCVReturnSuccess
    }
    
    override func draw(_ rect: NSRect) {
        // Noop.
        // We draw everything manually in animateOneFrame on a CVDisplayLink timer.
        // This stops ScreenSaverView from trying to draw stuff and creating OpenGL errors.
    }
    
    override func animateOneFrame() {
        super.animateOneFrame()
        
        let _ = drawView()
    }
    
    deinit {
        CVDisplayLinkStop(displayLink!)
        flux_destroy(flux!)
    }
    
    // The docs say I can override `resize`, but it’s not called...
    override func resizeSubviews(withOldSize oldSize: NSSize) {
        super.resizeSubviews(withOldSize: oldSize)
        
        // Detach the view from the OpenGL context, otherwise resizing breaks.
        // Lock things just in case
        openGLContext.lock()
        openGLContext.view = nil
        
        // First resize the frame
        setFrameSize(window!.frame.size)
        // Next resize the GL app
        let logical_size = frame.size
        let physical_size = window!.convertToBacking(frame).size;
        
//        Debug
//        print("resize:", logical_size, physical_size)
        
        flux_resize(
            flux,
            Float(logical_size.width),
            Float(logical_size.height),
            Float(physical_size.width),
            Float(physical_size.height)
        )
        
        openGLContext.view = self
        openGLContext.unlock()
    }
}
