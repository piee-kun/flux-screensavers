//
//  FluxView.swift
//  Flux
//
//  Created by Sander Melnikov on 05/02/2022.
//
import ScreenSaver
import Cocoa
import OpenGL.GL3

class FluxView: ScreenSaverView {
    var pixelFormat: NSOpenGLPixelFormat!
    var openGLContext: NSOpenGLContext!
    var displayLink: CVDisplayLink!
    var flux: OpaquePointer!
    var currentTime: Float = 0
    
    override init?(frame: NSRect, isPreview: Bool) {
        super.init(frame: frame, isPreview: isPreview)
        
        let attributes: [NSOpenGLPixelFormatAttribute] = [
            NSOpenGLPixelFormatAttribute(NSOpenGLPFAAccelerated),
            NSOpenGLPixelFormatAttribute(NSOpenGLPFADoubleBuffer),
            NSOpenGLPixelFormatAttribute(NSOpenGLPFAColorSize), 32,
            NSOpenGLPixelFormatAttribute(NSOpenGLPFAOpenGLProfile),
            NSOpenGLPixelFormatAttribute(NSOpenGLProfileVersion3_2Core),
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
            _self.currentTime += 1000.0 * 1.0 / (outputTime.rateScalar * Float(outputTime.videoTimeScale) / Float(outputTime.videoRefreshPeriod))
            
            // This stutters for some reason?
            // _self.currentTime = 1000.0 * Double(outputTime.videoTime) / Double(outputTime.videoTimeScale)
            
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
        
        let size = frame.size
        flux = flux_new(Float(size.width), Float(size.height))
        openGLContext?.unlock()
        
        CVDisplayLinkStart(displayLink!)
    }
    
    override func stopAnimation() {
        // Don’t call super. See startAnimation.
        CVDisplayLinkStop(displayLink!)
    }

    private func drawView() -> CVReturn {
        openGLContext.lock()
        openGLContext.makeCurrentContext()

        flux_animate(flux, currentTime)

        openGLContext.flushBuffer()
        openGLContext.unlock()

        return kCVReturnSuccess
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
        
        let size = window!.frame.size
        
        // Detach the view from the OpenGL context, otherwise resizing breaks.
        // Lock things just in case
        openGLContext.lock()
        openGLContext.view = nil
        
        // First resize the frame
        setFrameSize(window!.frame.size)
        // Next resize the GL app
        flux_resize(flux, Float(size.width), Float(size.height))
        
        openGLContext.view = self
        openGLContext.unlock()
    }
}
