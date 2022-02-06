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
    var pixelFormat: NSOpenGLPixelFormat?
    var openGLContext: NSOpenGLContext?
    var displayLink: CVDisplayLink?
    var currentTime = Float32(0.0)
    var flux: OpaquePointer?
//    var flux: UnsafeMutableRawPointer?
    var isReady = false
    
    // MARK: - Init / Setup
    override init?(frame: NSRect, isPreview: Bool) {
        super.init(frame: frame, isPreview: isPreview)
        
        let attributes: [NSOpenGLPixelFormatAttribute] = [
            UInt32(NSOpenGLPFAAccelerated),
            UInt32(NSOpenGLPFADoubleBuffer),
            UInt32(NSOpenGLPFAColorSize), UInt32(32),
            UInt32(NSOpenGLPFAOpenGLProfile),
            UInt32(NSOpenGLProfileVersion4_1Core),
            UInt32(0)
          ]
          guard let pixelFormat = NSOpenGLPixelFormat(attributes: attributes) else {
              print("Pixel format could not be constructed.")
              return nil
          }
          self.pixelFormat = pixelFormat
        guard let context = NSOpenGLContext(format: pixelFormat, share: nil) else {
            print("Context could not be constructed.")
            return nil
        }
        context.setValues([1], for: .swapInterval)
        self.openGLContext = context
        
        prepareOpenGL()
        
        
//        context.makeCurrentContext()
//        self.flux = flux_new(0.5)
        
//        context.cglContextObj!
//        self.flux = flux_new(context.cglContextObj!)
//        UnsafeMutableRawPointer(Unmanaged.passUnretained(context.cglContextObj).toOpaque())
        
    
        
//        let test = flux_new(32)
//        flux_animate(1.0)
        
    }
    
    // Debug in app
    required init?(coder decoder: NSCoder) {
        fatalError("init(coder:) has not been implemented")
    }
    
    func prepareOpenGL() {
        glClearColor(0.0, 0.0, 0.0, 1.0)

        let displayLinkOutputCallback: CVDisplayLinkOutputCallback = {(displayLink: CVDisplayLink, inNow: UnsafePointer<CVTimeStamp>, inOutputTime: UnsafePointer<CVTimeStamp>, flagsIn: CVOptionFlags, flagsOut: UnsafeMutablePointer<CVOptionFlags>, displayLinkContext: UnsafeMutableRawPointer?) -> CVReturn in
            let view = unsafeBitCast(displayLinkContext, to: FluxView.self)
            // Capture the current time in the currentTime property.
            // view.currentTime = inNow.pointee.videoTime / Int64(inNow.pointee.videoTimeScale)
            
            let result = view.drawView()

            //  We are going to assume that everything went well, and success as the CVReturn
            return result
        }
        
        CVDisplayLinkCreateWithActiveCGDisplays(&displayLink)
        CVDisplayLinkSetOutputCallback(displayLink!, displayLinkOutputCallback, UnsafeMutableRawPointer(Unmanaged.passUnretained(self).toOpaque()))
        
        CVDisplayLinkStart(displayLink!)

        //  Test render
//        _ = drawView()
    }
    
    override func lockFocus() {
      super.lockFocus()
      if openGLContext!.view != self {
        openGLContext!.view = self
      }
    }
    
    override func startAnimation() {
        super.startAnimation()
//        let size = self.frame.size
//        self.flux = flux_new(Float(size.width), Float(size.height))
    }

    
    override func draw(_ rect: NSRect) {
    }
    
//    fileprivate
    func drawView() -> CVReturn {
        guard let context = self.openGLContext else {
            Swift.print("Oh god")
            return kCVReturnError
        }

        self.currentTime += 1000.0 * 1.0 / 60.0

        context.lock()
        context.makeCurrentContext()

//        guard let flux = self.flux else {
//            Swift.print("Oh god")
//            return kCVReturnError
//        }
        if !self.isReady {
            let size = self.frame.size
            self.flux = flux_new(Float(size.width), Float(size.height))
            self.isReady = true
        }

        flux_animate(self.flux!, self.currentTime)

//        glClearColor(0.0, 0.0, 1.0, 1.0)
//        glClear(GLbitfield(GL_COLOR_BUFFER_BIT))


        context.flushBuffer()
        context.unlock()

        return kCVReturnSuccess


//        let background = NSBezierPath(rect: bounds)
//        NSColor.white.setFill()
//        background.fill()

    }
    
    override func animateOneFrame() {
        super.animateOneFrame()
        if openGLContext!.view != self {
            openGLContext!.view = self
          }
        
        // Is this actually doing anything?
//        if !CVDisplayLinkIsRunning(displayLink!) {
//            self.drawView()
//        }
    }
    
    deinit {
            //  Stop the display link.  A better place to stop the link is in
            //  the viewController or windowController within functions such as
            //  windowWillClose(_:)
            CVDisplayLinkStop(displayLink!)
//        flux_destroy(self.flux!)
        }
}
